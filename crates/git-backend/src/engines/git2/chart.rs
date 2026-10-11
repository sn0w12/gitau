//! Aggregated commit statistics for the history chart: additions/deletions
//! and commit counts rolled up into time buckets whose width adapts to the
//! repository's age. Reuses the history pipeline's revwalk, walk cache, and
//! per-commit diff stats.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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

/// Aggregates the chart and consults the on-disk caches.
///
/// Two files back this. `chart` holds finished buckets for one
/// `(tip, exclusions, bucket, merges)` tuple, so an unchanged tip answers
/// without walking or diffing anything. `points` holds per-commit stats keyed
/// by oid, which is what keeps a moved tip cheap: only commits that are
/// genuinely new pay for a tree diff.
pub fn history_chart_with_store(
    session: &Git2Session,
    query: &HistoryChartQuery,
    snapshot_id: SnapshotId,
    generation: Generation,
    cache: &Arc<Mutex<HistoryCache>>,
    base: Option<&std::path::Path>,
    repo_root: &std::path::Path,
) -> Result<HistoryChart> {
    let Some(base) = base else {
        return history_chart(session, query, snapshot_id, generation, cache);
    };
    let _ = std::fs::create_dir_all(base);

    let resolved = resolve_shape(session, query);
    let Some(shape) = resolved else {
        return history_chart(session, query, snapshot_id, generation, cache);
    };
    let Shape {
        tip: tip_bytes,
        exclusions,
    } = shape;
    let fingerprint = crate::runtime::stats_store::exclusions_fingerprint(&exclusions);

    let chart_path = crate::runtime::stats_store::chart_path(
        base,
        repo_root,
        query.bucket,
        query.include_merges,
    );
    if let Some(stored) = crate::runtime::stats_store::load_chart(
        &chart_path,
        tip_bytes,
        fingerprint,
        query.include_merges,
    ) {
        return Ok(HistoryChart {
            snapshot_id,
            generation,
            bucket_size: stored.bucket,
            buckets: stored.buckets,
            total_commits: stored.total_commits,
        });
    }

    // Merge the persisted per-commit points once per process. Entries are
    // keyed by oid, so nothing here can be wrong for a commit it names.
    {
        let mut guard = cache.lock().unwrap();
        if guard.claim_points_load() {
            let points_path = crate::runtime::stats_store::points_path(base, repo_root);
            if let Some(stored) = crate::runtime::stats_store::load_points(&points_path) {
                guard.extend_stats(stored.records);
            }
        }
    }

    let chart = history_chart(session, query, snapshot_id, generation, cache)?;

    let mut guard = cache.lock().unwrap();
    let walk_key = crate::runtime::history_cache::WalkKey {
        tip: Some(tip_bytes),
        exclusions: exclusions.clone(),
    };
    let Some(walk) = guard.walk(&walk_key) else {
        // No walk means an unborn HEAD, which produced no commits to record.
        return Ok(chart);
    };
    let order: Vec<[u8; 20]> = walk.iter().map(|oid| oid_bytes(*oid)).collect();
    guard.record_chart(
        tip_bytes,
        order,
        crate::runtime::history_cache::PendingChart {
            tip: tip_bytes,
            exclusions: fingerprint,
            requested_bucket: query.bucket,
            resolved_bucket: chart.bucket_size,
            include_merges: query.include_merges,
            total_commits: chart.total_commits,
            buckets: chart.buckets.clone(),
        },
    );
    drop(guard);

    // Persisting here rather than on a timer is deliberate: this branch only
    // runs after a full aggregation, where a write is small next to the tree
    // diffs that just happened. A chart served from the cache never writes.
    crate::runtime::stats_store::flush(base, repo_root, &cache.lock().unwrap());
    Ok(chart)
}

struct Shape {
    tip: [u8; 20],
    exclusions: Vec<[u8; 20]>,
}

/// Resolves the tip and hidden commits that identify an aggregate, or `None`
/// when the revision does not resolve and the caller should fall back to the
/// normal path and surface the error there.
fn resolve_shape(session: &Git2Session, query: &HistoryChartQuery) -> Option<Shape> {
    let resolved = session.with_repository(|repo| -> Result<Option<Shape>> {
        let tip = if crate::engines::git2::history::is_default_revision(query.revision.as_ref()) {
            crate::engines::git2::history::default_tip(repo)?
        } else {
            resolve_tip(repo, query.revision.as_ref()).ok()
        };
        let Some(tip) = tip else {
            return Ok(None);
        };
        let mut exclusions = Vec::with_capacity(query.exclude_reachable_from.len());
        for spec in &query.exclude_reachable_from {
            exclusions.push(oid_bytes(resolve_tip(repo, Some(spec))?));
        }
        Ok(Some(Shape {
            tip: oid_bytes(tip),
            exclusions,
        }))
    });
    resolved.ok().flatten()
}

pub fn history_chart(
    session: &Git2Session,
    query: &HistoryChartQuery,
    snapshot_id: SnapshotId,
    generation: Generation,
    cache: &Arc<Mutex<HistoryCache>>,
) -> Result<HistoryChart> {
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

        // The OID list is immutable once walked and is shared with the
        // history pages and the graph, which key on the same tip.
        let walk_key = crate::runtime::history_cache::WalkKey {
            tip: Some(oid_bytes(tip)),
            exclusions,
        };
        let oids: Arc<Vec<git2::Oid>> = {
            let mut guard = cache.lock().unwrap();
            match guard.walk(&walk_key) {
                Some(list) => list,
                None => {
                    let list = Arc::new(collect_walk(repo, tip, &query.exclude_reachable_from)?);
                    guard.store_walk(walk_key.clone(), list.clone());
                    list
                }
            }
        };

        // Per-commit stats come from the immutable stat cache first, then
        // the summaries cache the history pages fill (its additions,
        // deletions, and merge flag are exactly what a point needs); only
        // commits found in neither cache pay for a tree diff.
        let mut points: Vec<Option<CommitStatPoint>> = Vec::with_capacity(oids.len());
        let mut missing: Vec<usize> = Vec::new();
        {
            let mut guard = cache.lock().unwrap();
            for (idx, oid) in oids.iter().enumerate() {
                let bytes = oid_bytes(*oid);
                if let Some(point) = guard.stat(&bytes) {
                    points.push(Some(point));
                    continue;
                }
                match guard.summary(&bytes) {
                    Some(summary) => {
                        let point = CommitStatPoint {
                            time_seconds: summary.committer.time_seconds,
                            additions: summary.additions,
                            deletions: summary.deletions,
                            is_merge: summary.is_merge(),
                        };
                        guard.store_stat(bytes, point);
                        points.push(Some(point));
                    }
                    None => {
                        points.push(None);
                        missing.push(idx);
                    }
                }
            }
        }

        if !missing.is_empty() {
            // The cache lock is deliberately not held here. Diffing a large
            // missing set takes seconds, and holding the lock would stall
            // every other history read for this repository, including the
            // list the user is looking at.
            let computed = compute_missing(repo, &oids, &missing)?;
            let mut guard = cache.lock().unwrap();
            for (idx, point) in computed {
                guard.store_stat(oid_bytes(oids[idx]), point);
                points[idx] = Some(point);
            }
        }

        let included: Vec<CommitStatPoint> = points
            .into_iter()
            .flatten()
            .filter(|point| query.include_merges || !point.is_merge)
            .collect();
        if included.is_empty() {
            return Ok(HistoryChart {
                snapshot_id,
                generation,
                bucket_size: BucketSize::Day,
                buckets: Vec::new(),
                total_commits: 0,
            });
        }

        // Walk order is commit time, but points are keyed by committer time,
        // so the bounds are taken explicitly rather than from the ends.
        let oldest = included.iter().map(|p| p.time_seconds).min();
        let newest = included.iter().map(|p| p.time_seconds).max();
        let bucket_size = query
            .bucket
            .unwrap_or_else(|| select_auto(oldest.unwrap_or_default(), newest.unwrap_or_default()));
        let buckets = aggregate(&included, bucket_size)?;

        Ok(HistoryChart {
            snapshot_id,
            generation,
            total_commits: included.len() as u32,
            bucket_size,
            buckets,
        })
    })
}

/// Walks newest-first over everything reachable from `tip`.
fn collect_walk(
    repo: &git2::Repository,
    tip: git2::Oid,
    exclude_reachable_from: &[RevisionSpec],
) -> Result<Vec<git2::Oid>> {
    let mut walk = repo.revwalk()?;
    walk.push(tip)?;
    // TOPOLOGICAL keeps children before parents when timestamps tie.
    walk.set_sorting(Sort::TIME | Sort::TOPOLOGICAL)?;
    for spec in exclude_reachable_from {
        walk.hide(resolve_tip(repo, Some(spec))?)?;
    }
    walk.map(|info| {
        info.map_err(|e| GitError::Internal {
            message: e.to_string(),
        })
    })
    .collect()
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

/// Rolls points into zero-filled UTC buckets; bucket ends are exclusive, quiet
/// spans stay visible as empty buckets.
///
/// Points arrive in walk order, which is commit time and not necessarily the
/// committer time these points are keyed by, so ordering is recovered from a
/// `BTreeMap` over bucket boundaries. That costs one bucket's worth of memory
/// instead of a full sort of every point, which matters once the walk is no
/// longer capped.
fn aggregate(points: &[CommitStatPoint], size: BucketSize) -> Result<Vec<ChartBucket>> {
    let mut filled: BTreeMap<i64, ChartBucket> = BTreeMap::new();
    for point in points {
        let start = bucket_start(point.time_seconds, size)?;
        let bucket = filled.entry(start).or_insert(ChartBucket {
            start_seconds: start,
            end_seconds: 0,
            commits: 0,
            additions: 0,
            deletions: 0,
        });
        bucket.commits += 1;
        bucket.additions += point.additions;
        bucket.deletions += point.deletions;
    }

    let mut buckets: Vec<ChartBucket> = Vec::with_capacity(filled.len());
    let mut previous: Option<i64> = None;
    for (start, mut bucket) in filled {
        // Boundaries advance monotonically, so every boundary skipped between
        // two populated buckets is a quiet period that still needs to render.
        let mut cursor = match previous {
            Some(previous) => next_bucket_start(previous, size)?,
            None => start,
        };
        while cursor < start {
            buckets.push(ChartBucket {
                start_seconds: cursor,
                end_seconds: next_bucket_start(cursor, size)?,
                commits: 0,
                additions: 0,
                deletions: 0,
            });
            cursor = next_bucket_start(cursor, size)?;
        }
        bucket.end_seconds = next_bucket_start(bucket.start_seconds, size)?;
        previous = Some(bucket.start_seconds);
        buckets.push(bucket);
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

    /// `history_chart` takes the shared cache handle so it can release the
    /// lock across the tree diffs.
    fn cache() -> Arc<Mutex<HistoryCache>> {
        Arc::new(Mutex::new(HistoryCache::default()))
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
    fn aggregation_is_independent_of_input_order() {
        let mut ordered = vec![
            point(T, 3, 1),
            point(T + SECONDS_PER_HOUR, 2, 0),
            point(T + 3 * SECONDS_PER_HOUR, 5, 4),
        ];
        let shuffled = vec![ordered[2], ordered[0], ordered[1]];
        ordered.reverse();

        for size in [
            BucketSize::Hour,
            BucketSize::Day,
            BucketSize::Week,
            BucketSize::Month,
        ] {
            let a = aggregate(&ordered, size).unwrap();
            let b = aggregate(&shuffled, size).unwrap();
            assert_eq!(a, b, "bucket size {size:?}");
            assert_eq!(a.iter().map(|x| x.commits).sum::<u32>(), 3);
            assert_eq!(a.iter().map(|x| x.additions).sum::<u64>(), 10);
        }
    }

    #[test]
    fn merge_points_can_be_filtered_before_aggregation() {
        let slots = [
            Some(point(T, 3, 1)),
            Some(merge(T)),
            None, // never filled; dropped like any gap
        ];
        let filter = |include: bool| -> Vec<CommitStatPoint> {
            slots
                .iter()
                .flatten()
                .filter(|point| include || !point.is_merge)
                .copied()
                .collect()
        };
        assert_eq!(filter(false).len(), 1);
        assert_eq!(filter(true).len(), 2);
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
        let cache = cache();
        let chart = history_chart(
            &session,
            &HistoryChartQuery::default(),
            SnapshotId(0),
            Generation(1),
            &cache,
        )
        .unwrap();

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
            &cache,
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
            &cache,
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
        let cache = cache();

        let without = history_chart(
            &session,
            &HistoryChartQuery::default(),
            SnapshotId(0),
            Generation(1),
            &cache,
        )
        .unwrap();
        assert_eq!(without.total_commits, 3); // root + both branch tips; the merge is excluded

        let with_query = HistoryChartQuery {
            include_merges: true,
            ..Default::default()
        };
        let with =
            history_chart(&session, &with_query, SnapshotId(1), Generation(1), &cache).unwrap();
        assert_eq!(with.total_commits, 4);
        // Line totals unchanged: merge commits never contribute deltas.
        assert_eq!(
            with.buckets.iter().map(|b| b.additions).sum::<u64>(),
            without.buckets.iter().map(|b| b.additions).sum::<u64>()
        );
    }
}
