//! Aggregated commit statistics for the history chart: additions/deletions
//! and commit counts rolled up into time buckets whose width adapts to the
//! repository's age. Reuses the history pipeline's revwalk, walk cache, and
//! per-commit diff stats.

use std::path::PathBuf;

use chrono::{DateTime, Datelike, TimeZone, Utc};
use git2::Sort;
use rayon::prelude::*;

use crate::api::history::HistoryChartQuery;
use crate::domain::RevisionSpec;
use crate::domain::history::{BucketSize, ChartBucket, HistoryChart};
use crate::domain::{Generation, SnapshotId};
use crate::engines::git2::history::{commit_diff_stats, oid_bytes, resolve_tip};
use crate::engines::git2::local;
use crate::engines::git2::session::Git2Session;
use crate::error::{GitError, Result};
use crate::runtime::history_cache::{CommitStatPoint, HistoryCache};

const SECONDS_PER_HOUR: i64 = 3_600;
const SECONDS_PER_QUARTER_DAY: i64 = 6 * SECONDS_PER_HOUR;
const SECONDS_PER_HALF_DAY: i64 = 12 * SECONDS_PER_HOUR;
const SECONDS_PER_DAY: i64 = 24 * SECONDS_PER_HOUR;
const SECONDS_PER_TWO_DAYS: i64 = 2 * SECONDS_PER_DAY;
/// Twice per week.
const SECONDS_PER_HALF_WEEK: i64 = 3 * SECONDS_PER_DAY + SECONDS_PER_HALF_DAY;
const SECONDS_PER_WEEK: i64 = 7 * SECONDS_PER_DAY;
/// Julian year; only used for auto-selection thresholds, never for alignment.
const SECONDS_PER_YEAR: i64 = 365 * SECONDS_PER_DAY + SECONDS_PER_QUARTER_DAY;

pub fn history_chart(
    session: &Git2Session,
    query: &HistoryChartQuery,
    snapshot_id: SnapshotId,
    generation: Generation,
    cache: &mut HistoryCache,
) -> Result<HistoryChart> {
    let max_commits = query.max_commits() as usize;
    session.with_repository(|repo| {
        // Unborn HEAD charts nothing; mirrors the empty shape below instead
        // of failing with `invalid revision HEAD`.
        let tip = if crate::engines::git2::history::is_default_revision(query.revision.as_ref()) {
            match crate::engines::git2::history::default_tip(repo)? {
                Some(tip) => tip,
                None => {
                    return Ok(HistoryChart {
                        snapshot_id,
                        generation,
                        bucket_size: BucketSize::Day,
                        buckets: Vec::new(),
                        total_commits: 0,
                        truncated: false,
                    });
                }
            }
        } else {
            resolve_tip(repo, query.revision.as_ref())?
        };
        let mut exclusions = Vec::with_capacity(query.exclude_reachable_from.len());
        for spec in &query.exclude_reachable_from {
            exclusions.push(oid_bytes(resolve_tip(repo, Some(spec))?));
        }

        // The OID list is immutable once walked. Prefer the shared walk the
        // history pages built; only re-walk privately when the cached list
        // cannot cover the chart's window, and only store back a list long
        // enough to be worth sharing (a truncated chart walk would poison
        // pagination for pages that read the same entry).
        let walk_key = crate::runtime::history_cache::WalkKey {
            tip: Some(oid_bytes(tip)),
            exclusions,
        };
        let (oids, truncated) = if let Some(list) = cache.walk(&walk_key) {
            let cut = max_commits.min(list.len());
            let truncated = cut < list.len();
            (list[..cut].to_vec(), truncated)
        } else {
            let (oids, truncated) =
                collect_walk(repo, tip, &query.exclude_reachable_from, max_commits)?;
            // A complete list is always safe to share; a truncated one only
            // when it stops at the same 50k cap history pages use.
            if !truncated || oids.len() >= crate::runtime::history_cache::MAX_WALK_ENTRIES {
                cache.store_walk(walk_key.clone(), std::sync::Arc::new(oids.clone()));
            }
            (oids, truncated)
        };

        // Per-commit stats come from the immutable stat cache first, then
        // the summaries cache the history pages fill (its additions,
        // deletions, and merge flag are exactly what a point needs); only
        // commits found in neither cache pay for a tree diff.
        let mut points: Vec<Option<CommitStatPoint>> = Vec::with_capacity(oids.len());
        let mut missing: Vec<usize> = Vec::new();
        for (idx, oid) in oids.iter().enumerate() {
            let bytes = oid_bytes(*oid);
            if let Some(point) = cache.stat(&bytes) {
                points.push(Some(point));
                continue;
            }
            match cache.summary(&bytes) {
                Some(summary) => {
                    let point = CommitStatPoint {
                        time_seconds: summary.committer.time_seconds,
                        additions: summary.additions,
                        deletions: summary.deletions,
                        is_merge: summary.is_merge(),
                    };
                    cache.store_stat(bytes, point);
                    points.push(Some(point));
                }
                None => {
                    points.push(None);
                    missing.push(idx);
                }
            }
        }

        if !missing.is_empty() {
            let computed = compute_missing(repo, &oids, &missing)?;
            for (idx, point) in computed {
                cache.store_stat(oid_bytes(oids[idx]), point);
                points[idx] = Some(point);
            }
        }

        let included = included_points(points, query.include_merges);
        if included.is_empty() {
            return Ok(HistoryChart {
                snapshot_id,
                generation,
                bucket_size: BucketSize::Day,
                buckets: Vec::new(),
                total_commits: 0,
                truncated,
            });
        }

        let bucket_size = query.bucket.unwrap_or_else(|| {
            select_auto(
                included[0].time_seconds,
                included[included.len() - 1].time_seconds,
            )
        });
        let buckets = aggregate(&included, bucket_size)?;

        Ok(HistoryChart {
            snapshot_id,
            generation,
            total_commits: included.len() as u32,
            bucket_size,
            buckets,
            truncated,
        })
    })
}

/// Flattens the slot list, drops merges unless requested, and sorts by time.
fn included_points(
    points: Vec<Option<CommitStatPoint>>,
    include_merges: bool,
) -> Vec<CommitStatPoint> {
    let mut included: Vec<CommitStatPoint> = points
        .into_iter()
        .flatten()
        .filter(|point| include_merges || !point.is_merge)
        .collect();
    included.sort_unstable_by_key(|point| point.time_seconds);
    included
}

/// Walks newest-first, capped at `limit`; `truncated` reports whether older
/// commits exist beyond the cap.
fn collect_walk(
    repo: &git2::Repository,
    tip: git2::Oid,
    exclude_reachable_from: &[RevisionSpec],
    limit: usize,
) -> Result<(Vec<git2::Oid>, bool)> {
    let mut walk = repo.revwalk()?;
    walk.push(tip)?;
    // TOPOLOGICAL keeps children before parents when timestamps tie.
    walk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)?;
    for spec in exclude_reachable_from {
        walk.hide(resolve_tip(repo, Some(spec))?)?;
    }

    let mut oids = Vec::new();
    for info in &mut walk {
        if oids.len() >= limit {
            break;
        }
        oids.push(info.map_err(|e| GitError::Internal {
            message: e.to_string(),
        })?);
    }
    let truncated = oids.len() >= limit && walk.next().is_some();
    Ok((oids, truncated))
}

/// Computes stats for unknown commits in parallel over large chunks. A chunk
/// runs to completion on one rayon worker with one thread-local libgit2
/// handle, and chunks follow the walk's newest-first order: commit i's tree
/// was just loaded as commit i-1's parent tree, so libgit2's object cache
/// absorbs half the tree reads. Per-item scheduling would scatter neighbors
/// across handles and lose that reuse. Chunks are large enough that load
/// imbalance is negligible. Merge commits are never diffed: vs-first-parent
/// numbers would double-count merged branches.
fn compute_missing(
    repo: &git2::Repository,
    oids: &[git2::Oid],
    missing: &[usize],
) -> Result<Vec<(usize, CommitStatPoint)>> {
    const CHUNK_SIZE: usize = 16;
    let git_dir: PathBuf = repo.path().to_path_buf();
    let results: Vec<Result<Vec<(usize, CommitStatPoint)>>> = missing
        .par_chunks(CHUNK_SIZE)
        .map(|indices| {
            local::with_repository(&git_dir, |wrepo| {
                let mut chunk = Vec::with_capacity(indices.len());
                for idx in indices {
                    let commit = wrepo.find_commit(oids[*idx])?;
                    let is_merge = commit.parent_count() > 1;
                    let time_seconds = commit.time().seconds();
                    let (additions, deletions) = if is_merge {
                        (0, 0)
                    } else {
                        let (_, additions, deletions) = commit_diff_stats(wrepo, &commit)?;
                        (additions, deletions)
                    };
                    chunk.push((
                        *idx,
                        CommitStatPoint {
                            time_seconds,
                            additions,
                            deletions,
                            is_merge,
                        },
                    ));
                }
                Ok(chunk)
            })
        })
        .collect();
    let mut out = Vec::new();
    for chunk in results {
        out.extend(chunk?);
    }
    Ok(out)
}

/// Auto-selection thresholds by time since the first commit. Every tier keeps
/// the bucket count in roughly the same 100-1100 band, so bar density stays
/// even as a repository ages:
/// monthly 10y+, weekly 6y+, half-weekly 3y+, two-daily 1y+, daily 180d+,
/// half-daily 90d+, quarter-daily 14d+, hourly below.
fn select_auto(first_seconds: i64, last_seconds: i64) -> BucketSize {
    let span = (last_seconds - first_seconds).max(0);
    if span >= 10 * SECONDS_PER_YEAR {
        BucketSize::Month
    } else if span >= 6 * SECONDS_PER_YEAR {
        BucketSize::Week
    } else if span >= 3 * SECONDS_PER_YEAR {
        BucketSize::HalfWeek
    } else if span >= SECONDS_PER_YEAR {
        BucketSize::TwoDays
    } else if span >= 180 * SECONDS_PER_DAY {
        BucketSize::Day
    } else if span >= 90 * SECONDS_PER_DAY {
        BucketSize::HalfDay
    } else if span >= 14 * SECONDS_PER_DAY {
        BucketSize::QuarterDay
    } else {
        BucketSize::Hour
    }
}

/// Inclusive floor boundary of the bucket containing `seconds`, in UTC.
fn bucket_start(seconds: i64, size: BucketSize) -> Result<i64> {
    Ok(match size {
        BucketSize::Hour => align_epoch(seconds, SECONDS_PER_HOUR),
        BucketSize::QuarterDay => align_epoch(seconds, SECONDS_PER_QUARTER_DAY),
        BucketSize::HalfDay => align_epoch(seconds, SECONDS_PER_HALF_DAY),
        BucketSize::Day => align_epoch(seconds, SECONDS_PER_DAY),
        BucketSize::TwoDays => align_epoch(seconds, SECONDS_PER_TWO_DAYS),
        // Fixed half-week blocks from the epoch; boundaries fall on the same
        // clock time every 3.5 days.
        BucketSize::HalfWeek => align_epoch(seconds, SECONDS_PER_HALF_WEEK),
        BucketSize::Week => {
            let day_start = align_epoch(seconds, SECONDS_PER_DAY);
            // Unix day 0 was a Thursday; step back to the week's Monday.
            let days = day_start.div_euclid(SECONDS_PER_DAY);
            day_start - (days + 3).rem_euclid(7) * SECONDS_PER_DAY
        }
        BucketSize::Month => month_floor(seconds, 1)?,
        BucketSize::Quarter => month_floor(seconds, 3)?,
    })
}

/// Exclusive end of the bucket starting at `start`.
fn next_bucket_start(start: i64, size: BucketSize) -> Result<i64> {
    Ok(match size {
        BucketSize::Hour => start + SECONDS_PER_HOUR,
        BucketSize::QuarterDay => start + SECONDS_PER_QUARTER_DAY,
        BucketSize::HalfDay => start + SECONDS_PER_HALF_DAY,
        BucketSize::Day => start + SECONDS_PER_DAY,
        BucketSize::TwoDays => start + SECONDS_PER_TWO_DAYS,
        BucketSize::HalfWeek => start + SECONDS_PER_HALF_WEEK,
        BucketSize::Week => start + SECONDS_PER_WEEK,
        BucketSize::Month => advance_months(start, 1)?,
        BucketSize::Quarter => advance_months(start, 3)?,
    })
}

fn align_epoch(seconds: i64, unit: i64) -> i64 {
    seconds - seconds.rem_euclid(unit)
}

/// Start of the month-group (`step` months wide) containing `seconds`.
fn month_floor(seconds: i64, step: u32) -> Result<i64> {
    let dt = date_time(seconds)?;
    let month_index = ((i64::from(dt.month()) - 1) / i64::from(step)) * i64::from(step) + 1;
    utc_month_start(dt.year(), month_index as u32)
}

fn advance_months(start: i64, delta: u32) -> Result<i64> {
    let dt = date_time(start)?;
    let total = i64::from(dt.year()) * 12 + i64::from(dt.month()) - 1 + i64::from(delta);
    utc_month_start((total / 12) as i32, (total % 12) as u32 + 1)
}

fn utc_month_start(year: i32, month: u32) -> Result<i64> {
    Utc.with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .earliest()
        .map(|dt| dt.timestamp())
        .ok_or_else(|| GitError::internal("invalid calendar date"))
}

fn date_time(seconds: i64) -> Result<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp(seconds, 0)
        .ok_or_else(|| GitError::internal("timestamp out of range"))
}

/// Rolls sorted points into zero-filled UTC buckets; bucket ends are
/// exclusive, quiet spans stay visible as empty buckets.
fn aggregate(points: &[CommitStatPoint], size: BucketSize) -> Result<Vec<ChartBucket>> {
    let first = points[0].time_seconds;
    let last = points[points.len() - 1].time_seconds;

    let mut buckets = Vec::new();
    let mut cursor = 0usize;
    let mut start = bucket_start(first, size)?;
    while start <= last {
        let end = next_bucket_start(start, size)?;
        let mut bucket = ChartBucket {
            start_seconds: start,
            end_seconds: end,
            commits: 0,
            additions: 0,
            deletions: 0,
        };
        // Everything before `start` was consumed by earlier buckets because
        // boundaries advance monotonically from below the first timestamp.
        while cursor < points.len() && points[cursor].time_seconds < end {
            bucket.commits += 1;
            bucket.additions += points[cursor].additions;
            bucket.deletions += points[cursor].deletions;
            cursor += 1;
        }
        buckets.push(bucket);
        start = end;
    }
    Ok(buckets)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(time: i64, adds: u64, dels: u64) -> CommitStatPoint {
        CommitStatPoint {
            time_seconds: time,
            additions: adds,
            deletions: dels,
            is_merge: false,
        }
    }

    fn merge(time: i64) -> CommitStatPoint {
        CommitStatPoint {
            time_seconds: time,
            additions: 0,
            deletions: 0,
            is_merge: true,
        }
    }

    /// 2024-03-15T12:34:56Z
    const T: i64 = 1_710_506_096;

    #[test]
    fn auto_selection_follows_age_thresholds() {
        // Every tier boundary, plus a sample inside each tier.
        assert_eq!(select_auto(T, T + 11 * SECONDS_PER_YEAR), BucketSize::Month);
        assert_eq!(select_auto(T, T + 10 * SECONDS_PER_YEAR), BucketSize::Month);
        assert_eq!(
            select_auto(T, T + 10 * SECONDS_PER_YEAR - SECONDS_PER_DAY),
            BucketSize::Week
        );
        assert_eq!(select_auto(T, T + 6 * SECONDS_PER_YEAR), BucketSize::Week);
        assert_eq!(
            select_auto(T, T + 6 * SECONDS_PER_YEAR - SECONDS_PER_DAY),
            BucketSize::HalfWeek
        );
        assert_eq!(
            select_auto(T, T + 3 * SECONDS_PER_YEAR),
            BucketSize::HalfWeek
        );
        assert_eq!(
            select_auto(T, T + 3 * SECONDS_PER_YEAR - SECONDS_PER_DAY),
            BucketSize::TwoDays
        );
        assert_eq!(
            select_auto(T, T + 2 * SECONDS_PER_YEAR),
            BucketSize::TwoDays
        );
        assert_eq!(select_auto(T, T + SECONDS_PER_YEAR), BucketSize::TwoDays);
        assert_eq!(
            select_auto(T, T + SECONDS_PER_YEAR - SECONDS_PER_DAY),
            BucketSize::Day
        );
        assert_eq!(select_auto(T, T + 180 * SECONDS_PER_DAY), BucketSize::Day);
        assert_eq!(
            select_auto(T, T + 90 * SECONDS_PER_DAY),
            BucketSize::HalfDay
        );
        assert_eq!(
            select_auto(T, T + 90 * SECONDS_PER_DAY - SECONDS_PER_DAY),
            BucketSize::QuarterDay
        );
        assert_eq!(
            select_auto(T, T + 14 * SECONDS_PER_DAY),
            BucketSize::QuarterDay
        );
        assert_eq!(
            select_auto(T, T + 14 * SECONDS_PER_DAY - SECONDS_PER_HOUR),
            BucketSize::Hour
        );
        assert_eq!(select_auto(T, T), BucketSize::Hour);
    }

    #[test]
    fn hour_day_alignment_is_epoch_flooring() {
        assert_eq!(
            bucket_start(T, BucketSize::Hour).unwrap(),
            align_epoch(T, 3600)
        );
        assert_eq!(
            bucket_start(T, BucketSize::Day).unwrap(),
            align_epoch(T, 86400)
        );
    }

    #[test]
    fn weeks_align_to_mondays() {
        let start = bucket_start(T, BucketSize::Week).unwrap();
        let dt = DateTime::<Utc>::from_timestamp(start, 0).unwrap();
        assert_eq!(dt.weekday(), chrono::Weekday::Mon);
        assert!(start <= T && T - start < SECONDS_PER_WEEK);
        // Pre-epoch timestamps still floor onto a Monday.
        let ancient = bucket_start(-1_000_000_000, BucketSize::Week).unwrap();
        let dt = DateTime::<Utc>::from_timestamp(ancient, 0).unwrap();
        assert_eq!(dt.weekday(), chrono::Weekday::Mon);
    }

    #[test]
    fn month_and_quarter_floor_to_calendar_starts() {
        // 2024-03-15 -> March 1st.
        assert_eq!(bucket_start(T, BucketSize::Month).unwrap(), 1_709_251_200);
        // ...and Q1 starts even earlier (January 1st).
        assert_eq!(bucket_start(T, BucketSize::Quarter).unwrap(), 1_704_067_200);
    }

    #[test]
    fn calendar_buckets_step_across_year_boundaries() {
        let dec = utc_month_start(2025, 12).unwrap();
        let next = next_bucket_start(dec, BucketSize::Month).unwrap();
        assert_eq!(next, utc_month_start(2026, 1).unwrap());
        let q = utc_month_start(2025, 11).unwrap(); // Q4
        assert_eq!(
            next_bucket_start(q, BucketSize::Quarter).unwrap(),
            utc_month_start(2026, 2).unwrap()
        );
    }

    #[test]
    fn aggregation_zero_fills_gaps_and_keeps_ends_exclusive() {
        let points = vec![
            point(T, 10, 2),
            point(T + SECONDS_PER_HOUR, 5, 1),
            point(T + SECONDS_PER_DAY * 3, 7, 0), // two empty days in between
        ];
        let buckets = aggregate(&points, BucketSize::Hour).unwrap();
        assert_eq!(buckets.len(), 73); // hours 0..72 inclusive
        assert_eq!(buckets[0].commits, 1);
        assert_eq!(buckets[0].additions, 10);
        assert_eq!(buckets[1].commits, 1);
        assert_eq!(buckets[1].end_seconds - buckets[1].start_seconds, 3600);
        assert_eq!(buckets[2].commits, 0);
        assert_eq!(buckets[72].additions, 7);
        assert!(
            buckets
                .windows(2)
                .all(|w| w[0].start_seconds < w[1].start_seconds)
        );
    }

    #[test]
    fn merges_are_filtered_until_requested() {
        let slots = vec![
            Some(point(T, 3, 1)),
            Some(merge(T)),
            None, // never filled; dropped like any gap
        ];
        assert_eq!(included_points(slots.clone(), false).len(), 1);
        let with_merges = included_points(slots, true);
        assert_eq!(with_merges.len(), 2);
        // Sorted chronologically regardless of input order.
        assert!(with_merges[0].time_seconds <= with_merges[1].time_seconds);
    }

    /// Commits `contents` as file.txt on `refname` (e.g. "HEAD" or
    /// "refs/heads/side") authored at exactly `seconds`.
    fn commit_at(
        repo: &git2::Repository,
        refname: &str,
        parents: &[&git2::Oid],
        message: &str,
        seconds: i64,
        contents: &str,
    ) -> git2::Oid {
        let sig =
            git2::Signature::new("tester", "tester@example.com", &git2::Time::new(seconds, 0))
                .unwrap();
        std::fs::write(repo.workdir().unwrap().join("file.txt"), contents).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("file.txt")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let parent_commits: Vec<git2::Commit> = parents
            .iter()
            .map(|oid| repo.find_commit(**oid).unwrap())
            .collect();
        let parent_refs: Vec<&git2::Commit> = parent_commits.iter().collect();
        repo.commit(Some(refname), &sig, &sig, message, &tree, &parent_refs)
            .unwrap()
    }

    fn init_repo(dir: &std::path::Path) -> git2::Repository {
        let repo = git2::Repository::init(dir).unwrap();
        {
            let mut config = repo.config().unwrap();
            config.set_str("user.name", "tester").unwrap();
            config.set_str("user.email", "tester@example.com").unwrap();
        }
        repo
    }

    #[test]
    fn charts_a_real_repository_with_auto_selected_buckets() {
        let temp = tempfile::tempdir().unwrap();
        let repo = init_repo(temp.path());
        // Day 0 (+3/-1), day 2 (+1/-4), day 40 (+3/-1): a 40-day span lands
        // in the quarter-day tier of the age ladder.
        let base = align_epoch(T, SECONDS_PER_DAY);
        let c1 = commit_at(&repo, "HEAD", &[], "first", base, "a\nb\nc");
        let c2 = commit_at(
            &repo,
            "HEAD",
            &[&c1],
            "second",
            base + 2 * SECONDS_PER_DAY,
            "z\nb",
        );
        commit_at(
            &repo,
            "HEAD",
            &[&c2],
            "third",
            base + 40 * SECONDS_PER_DAY,
            "z\ny\nx",
        );

        let session = Git2Session::new(temp.path().to_path_buf());
        let mut cache = HistoryCache::default();
        let chart = history_chart(
            &session,
            &HistoryChartQuery::default(),
            SnapshotId(0),
            Generation(1),
            &mut cache,
        )
        .unwrap();

        assert!(!chart.truncated);
        assert_eq!(chart.total_commits, 3);
        // A 40-day span lands in the quarter-day tier (14d..90d).
        assert_eq!(chart.bucket_size, BucketSize::QuarterDay);
        // Zero-filled quarter-days 0..=160: commits on exact UTC midnights
        // align to quarter-day boundaries.
        assert_eq!(chart.buckets.len(), 161);
        assert_eq!(chart.buckets[0].commits, 1);
        // The root commit diffs against the empty tree: every line is an
        // addition and nothing is deleted.
        assert_eq!(chart.buckets[0].additions, 3);
        assert_eq!(chart.buckets[0].deletions, 0);
        assert_eq!(chart.buckets[1].commits, 0);
        assert_eq!(chart.buckets[8].commits, 1); // day 2
        assert_eq!(chart.buckets[160].commits, 1); // day 40
        // Every commit landed in exactly one bucket.
        assert_eq!(
            chart
                .buckets
                .iter()
                .map(|bucket| bucket.commits)
                .sum::<u32>(),
            3
        );

        // A second run is served from the immutable stat cache and produces
        // identical buckets.
        let again = history_chart(
            &session,
            &HistoryChartQuery::default(),
            SnapshotId(1),
            Generation(1),
            &mut cache,
        )
        .unwrap();
        assert_eq!(again.buckets, chart.buckets);

        // An explicit unit overrides the auto selection.
        let weekly_query = HistoryChartQuery {
            bucket: Some(BucketSize::Week),
            ..Default::default()
        };
        let weekly = history_chart(
            &session,
            &weekly_query,
            SnapshotId(2),
            Generation(1),
            &mut cache,
        )
        .unwrap();
        assert_eq!(weekly.bucket_size, BucketSize::Week);
        // Day 0 (2024-03-15) is a Friday, so its week bucket begins on the
        // preceding Monday and day 40 lands in the 7th week overall.
        assert_eq!(weekly.buckets.len(), 7);
        assert_eq!(
            weekly
                .buckets
                .iter()
                .map(|bucket| bucket.commits)
                .sum::<u32>(),
            3
        );
    }

    #[test]
    fn merges_are_excluded_by_default_and_counted_when_requested() {
        let temp = tempfile::tempdir().unwrap();
        let repo = init_repo(temp.path());
        let base = align_epoch(T, SECONDS_PER_DAY);
        let root = commit_at(&repo, "HEAD", &[], "root", base, "one\n");

        // Commit on a side branch and on master without ever checking out;
        // the worktree state is irrelevant to the revwalk.
        let side = commit_at(
            &repo,
            "refs/heads/side",
            &[&root],
            "side work",
            base + 120,
            "one\ntwo\nthree\n",
        );
        let main_tip = commit_at(
            &repo,
            "HEAD",
            &[&root],
            "main work",
            base + 60,
            "one\nfour\n",
        );
        let sig = git2::Signature::new(
            "tester",
            "tester@example.com",
            &git2::Time::new(base + 180, 0),
        )
        .unwrap();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "merge side",
            &repo.find_commit(main_tip).unwrap().tree().unwrap(),
            &[
                &repo.find_commit(main_tip).unwrap(),
                &repo.find_commit(side).unwrap(),
            ],
        )
        .unwrap();

        let session = Git2Session::new(temp.path().to_path_buf());
        let mut cache = HistoryCache::default();

        let without = history_chart(
            &session,
            &HistoryChartQuery::default(),
            SnapshotId(0),
            Generation(1),
            &mut cache,
        )
        .unwrap();
        assert_eq!(without.total_commits, 3); // root + both branch tips; the merge is excluded

        let with_query = HistoryChartQuery {
            include_merges: true,
            ..Default::default()
        };
        let with = history_chart(
            &session,
            &with_query,
            SnapshotId(1),
            Generation(1),
            &mut cache,
        )
        .unwrap();
        assert_eq!(with.total_commits, 4);
        // Line totals unchanged: merge commits never contribute deltas.
        assert_eq!(
            with.buckets.iter().map(|b| b.additions).sum::<u64>(),
            without.buckets.iter().map(|b| b.additions).sum::<u64>()
        );
    }
}
