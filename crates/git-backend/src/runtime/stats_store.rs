//! On-disk caches for history chart data.
//!
//! A [`CommitStatPoint`] is a pure function of its commit oid: committer time,
//! parent count, and the first-parent tree diff are all immutable once the
//! object exists. Nothing here expires or is invalidated by a repository
//! generation bump, because there is nothing to invalidate. The worst a stale
//! file can do is be missing commits that are newer than it, which surfaces as
//! a miss and a recompute, never as a wrong number.
//!
//! Two files back one repository:
//!
//! * `points` holds one record per commit in walk order. It is the expensive
//!   input (a tree diff per commit) and is keyed by oid, so it survives tip
//!   movement, rebases, and merges without any bookkeeping.
//! * `chart` holds finished buckets for one `(tip, exclusions, bucket size,
//!   merges)` tuple. It is small and makes an unchanged tip a sub-millisecond
//!   read that never walks the history at all.
//!
//! Every record carries its own oid and both files checksum their bodies, so a
//! truncated or corrupt file is discarded rather than misread. Layouts are
//! little-endian and fixed width to stay seekable and append-friendly.

use std::path::{Path, PathBuf};

use crate::domain::history::{BucketSize, ChartBucket};
use crate::runtime::disk::{fnv1a64, sanitize_component, write_atomic};
use crate::runtime::history_cache::CommitStatPoint;

const POINTS_MAGIC: &[u8; 4] = b"GTCS";
const POINTS_VERSION: u32 = 1;
const POINTS_HEADER_LEN: usize = 48;
const POINT_RECORD_LEN: usize = 37;
/// `oid + time + additions + deletions + flags`.
const POINT_BODY_LEN: usize = 20 + 8 + 4 + 4 + 1;

const CHART_MAGIC: &[u8; 4] = b"GTCH";
const CHART_VERSION: u32 = 1;
const CHART_HEADER_LEN: usize = 50;
const BUCKET_RECORD_LEN: usize = 36;
const OID_LEN: usize = 20;

/// Additions and deletions are stored as `u32`. A single commit exceeding four
/// billion changed lines is not a real shape, and clamping keeps the record at
/// 37 bytes.
const LINE_COUNT_MAX: u32 = u32::MAX;

/// Per-commit chart inputs recovered from disk, in the walk order they were
/// written.
pub struct StoredPoints {
    pub tip: [u8; OID_LEN],
    pub records: Vec<([u8; OID_LEN], CommitStatPoint)>,
}

/// Finished buckets plus the shape they were aggregated into. The requested
/// bucket size lives in the filename, so an auto-selected chart can still be
/// found without walking the history to discover which size it resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredChart {
    pub bucket: BucketSize,
    pub total_commits: u32,
    pub buckets: Vec<ChartBucket>,
}

pub fn points_path(base: &Path, repo_root: &Path) -> PathBuf {
    base.join(format!("points-{}.bin", repo_slug(repo_root)))
}

pub fn chart_path(
    base: &Path,
    repo_root: &Path,
    requested: Option<BucketSize>,
    include_merges: bool,
) -> PathBuf {
    base.join(format!(
        "chart-{}-{}.bin",
        repo_slug(repo_root),
        chart_slug(requested, include_merges)
    ))
}

/// Readable prefix plus a 64-bit hash of the full path. The prefix keeps the
/// file identifiable when browsing the cache dir; the hash is what actually
/// keeps two repositories apart.
fn repo_slug(repo_root: &Path) -> String {
    let text = repo_root.to_string_lossy();
    format!(
        "{}-{:016x}",
        sanitize_component(&text),
        fnv1a64(text.as_bytes())
    )
}

fn chart_slug(requested: Option<BucketSize>, include_merges: bool) -> String {
    let bucket = match requested {
        Some(bucket) => format!("{:02}", bucket_code(bucket)),
        None => "auto".to_owned(),
    };
    format!("{bucket}-{}", u8::from(include_merges))
}

fn bucket_code(bucket: BucketSize) -> u8 {
    match bucket {
        BucketSize::Hour => 0,
        BucketSize::QuarterDay => 1,
        BucketSize::HalfDay => 2,
        BucketSize::Day => 3,
        BucketSize::TwoDays => 4,
        BucketSize::HalfWeek => 5,
        BucketSize::Week => 6,
        BucketSize::Month => 7,
        BucketSize::Quarter => 8,
    }
}

fn bucket_from_code(code: u8) -> Option<BucketSize> {
    Some(match code {
        0 => BucketSize::Hour,
        1 => BucketSize::QuarterDay,
        2 => BucketSize::HalfDay,
        3 => BucketSize::Day,
        4 => BucketSize::TwoDays,
        5 => BucketSize::HalfWeek,
        6 => BucketSize::Week,
        7 => BucketSize::Month,
        8 => BucketSize::Quarter,
        _ => return None,
    })
}

/// Identifies the hidden set. Two queries that hide different commits
/// aggregate different walks even from the same tip, so the fingerprint has to
/// be part of the chart cache key.
pub fn exclusions_fingerprint(exclusions: &[[u8; OID_LEN]]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for oid in exclusions {
        for byte in oid {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_i64(out: &mut Vec<u8>, value: i64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn read_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
}

fn read_i64(bytes: &[u8], at: usize) -> Option<i64> {
    let slice = bytes.get(at..at + 8)?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(slice);
    Some(i64::from_le_bytes(buf))
}

fn read_u64(bytes: &[u8], at: usize) -> Option<u64> {
    let slice = bytes.get(at..at + 8)?;
    let mut buf = [0u8; 8];
    buf.copy_from_slice(slice);
    Some(u64::from_le_bytes(buf))
}

fn read_oid(bytes: &[u8], at: usize) -> Option<[u8; OID_LEN]> {
    let slice = bytes.get(at..at + OID_LEN)?;
    let mut oid = [0u8; OID_LEN];
    oid.copy_from_slice(slice);
    Some(oid)
}

pub fn load_points(path: &Path) -> Option<StoredPoints> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.len() < POINTS_HEADER_LEN {
        return None;
    }
    if &bytes[0..4] != POINTS_MAGIC {
        return None;
    }
    if read_u32(&bytes, 4)? != POINTS_VERSION {
        return None;
    }
    if read_u32(&bytes, 8)? as usize != OID_LEN {
        return None;
    }
    let count = read_u32(&bytes, 12)? as usize;
    let tip = read_oid(&bytes, 16)?;
    let body_len = read_u64(&bytes, 36)? as usize;
    let checksum = read_u32(&bytes, 44)?;

    // The declared length has to agree with the record count *and* with the
    // bytes actually present, so a truncated file is rejected before any of it
    // is parsed.
    if body_len != count * POINT_BODY_LEN {
        return None;
    }
    let body = bytes.get(POINTS_HEADER_LEN..POINTS_HEADER_LEN + body_len)?;
    if fnv1a64(body) as u32 != checksum {
        return None;
    }

    let mut records = Vec::with_capacity(count);
    for index in 0..count {
        let at = index * POINT_RECORD_LEN;
        let oid = read_oid(body, at)?;
        let time_seconds = read_i64(body, at + 20)?;
        let additions = read_u32(body, at + 28)?;
        let deletions = read_u32(body, at + 32)?;
        let flags = *body.get(at + 36)?;
        records.push((
            oid,
            CommitStatPoint {
                time_seconds,
                additions: u64::from(additions),
                deletions: u64::from(deletions),
                is_merge: flags & 1 != 0,
            },
        ));
    }

    Some(StoredPoints { tip, records })
}

pub fn save_points(
    path: &Path,
    tip: [u8; OID_LEN],
    records: &[([u8; OID_LEN], CommitStatPoint)],
) -> std::io::Result<()> {
    let mut body = Vec::with_capacity(records.len() * POINT_RECORD_LEN);
    for (oid, point) in records {
        body.extend_from_slice(oid);
        put_i64(&mut body, point.time_seconds);
        put_u32(
            &mut body,
            point.additions.min(u64::from(LINE_COUNT_MAX)) as u32,
        );
        put_u32(
            &mut body,
            point.deletions.min(u64::from(LINE_COUNT_MAX)) as u32,
        );
        body.push(u8::from(point.is_merge));
    }

    let mut out = Vec::with_capacity(POINTS_HEADER_LEN + body.len());
    out.extend_from_slice(POINTS_MAGIC);
    put_u32(&mut out, POINTS_VERSION);
    put_u32(&mut out, OID_LEN as u32);
    put_u32(&mut out, records.len() as u32);
    out.extend_from_slice(&tip);
    put_u64(&mut out, body.len() as u64);
    put_u32(&mut out, fnv1a64(&body) as u32);
    out.extend_from_slice(&body);

    write_atomic(path, &out)
}

fn chart_matches(
    bytes: &[u8],
    tip: [u8; OID_LEN],
    exclusions: u64,
    include_merges: bool,
) -> Option<StoredChart> {
    if bytes.len() < CHART_HEADER_LEN || &bytes[0..4] != CHART_MAGIC {
        return None;
    }
    if read_u32(bytes, 4)? != CHART_VERSION {
        return None;
    }
    if bytes[9] != u8::from(include_merges) {
        return None;
    }
    if read_oid(bytes, 10)? != tip {
        return None;
    }
    if read_u64(bytes, 30)? != exclusions {
        return None;
    }
    let total_commits = read_u32(bytes, 38)?;
    let count = read_u32(bytes, 42)? as usize;
    let body = bytes.get(CHART_HEADER_LEN..CHART_HEADER_LEN + count * BUCKET_RECORD_LEN)?;
    if fnv1a64(body) as u32 != read_u32(bytes, 46)? {
        return None;
    }
    let bucket = bucket_from_code(bytes[8])?;

    let mut buckets = Vec::with_capacity(count);
    for index in 0..count {
        let at = index * BUCKET_RECORD_LEN;
        buckets.push(ChartBucket {
            start_seconds: read_i64(body, at)?,
            end_seconds: read_i64(body, at + 8)?,
            commits: read_u32(body, at + 16)?,
            additions: read_u64(body, at + 20)?,
            deletions: read_u64(body, at + 28)?,
        });
    }
    Some(StoredChart {
        bucket,
        total_commits,
        buckets,
    })
}

pub fn load_chart(
    path: &Path,
    tip: [u8; OID_LEN],
    exclusions: u64,
    include_merges: bool,
) -> Option<StoredChart> {
    let bytes = std::fs::read(path).ok()?;
    chart_matches(&bytes, tip, exclusions, include_merges)
}

pub fn save_chart(
    path: &Path,
    tip: [u8; OID_LEN],
    exclusions: u64,
    bucket: BucketSize,
    include_merges: bool,
    total_commits: u32,
    buckets: &[ChartBucket],
) -> std::io::Result<()> {
    let mut body = Vec::with_capacity(buckets.len() * BUCKET_RECORD_LEN);
    for bucket in buckets {
        put_i64(&mut body, bucket.start_seconds);
        put_i64(&mut body, bucket.end_seconds);
        put_u32(&mut body, bucket.commits);
        put_u64(&mut body, bucket.additions);
        put_u64(&mut body, bucket.deletions);
    }

    let mut out = Vec::with_capacity(CHART_HEADER_LEN + body.len());
    out.extend_from_slice(CHART_MAGIC);
    put_u32(&mut out, CHART_VERSION);
    out.push(bucket_code(bucket));
    out.push(u8::from(include_merges));
    out.extend_from_slice(&tip);
    put_u64(&mut out, exclusions);
    put_u32(&mut out, total_commits);
    put_u32(&mut out, buckets.len() as u32);
    put_u32(&mut out, fnv1a64(&body) as u32);
    out.extend_from_slice(&body);

    write_atomic(path, &out)
}

/// Rewrites both files from the cache's current contents. Best effort: a
/// failure here costs recomputation on the next launch, never correctness.
///
/// Pruning keys off mtime, so a repository that is never recomputed eventually
/// ages out and is recomputed once. Bumping the mtime on every read was tried
/// and cost more than it saved: two file opens on Windows are milliseconds,
/// which is most of a cache hit.
pub fn flush(base: &Path, repo_root: &Path, cache: &crate::runtime::history_cache::HistoryCache) {
    let _ = std::fs::create_dir_all(base);
    if let Some((tip, order)) = cache.chart_walk() {
        let records = cache.stats_in_walk_order(order);
        let path = points_path(base, repo_root);
        if let Err(error) = save_points(&path, tip, &records) {
            log::warn!("history stat cache write failed at {path:?}: {error}");
        }
    }
    for ((requested, merges), chart) in cache.pending_charts() {
        let path = chart_path(base, repo_root, requested, merges);
        if let Err(error) = save_chart(
            &path,
            chart.tip,
            chart.exclusions,
            chart.resolved_bucket,
            merges,
            chart.total_commits,
            &chart.buckets,
        ) {
            log::warn!("history chart cache write failed at {path:?}: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(time: i64, adds: u64, dels: u64, is_merge: bool) -> CommitStatPoint {
        CommitStatPoint {
            time_seconds: time,
            additions: adds,
            deletions: dels,
            is_merge,
        }
    }

    #[test]
    fn points_round_trip_in_walk_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = points_path(dir.path(), Path::new("/repos/example"));
        let tip = [7u8; OID_LEN];
        let records = vec![
            ([9u8; OID_LEN], point(100, 5, 2, false)),
            ([8u8; OID_LEN], point(50, 0, 0, true)),
            ([1u8; OID_LEN], point(0, u64::from(u32::MAX), 1, false)),
        ];

        save_points(&path, tip, &records).unwrap();
        let loaded = load_points(&path).unwrap();

        assert_eq!(loaded.tip, tip);
        assert_eq!(loaded.records, records);
    }

    #[test]
    fn truncated_points_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = points_path(dir.path(), Path::new("/repos/example"));
        let records = vec![([1u8; OID_LEN], point(10, 1, 1, false))];
        save_points(&path, [2u8; OID_LEN], &records).unwrap();

        let full = std::fs::read(&path).unwrap();
        // Losing the tail must not yield a shorter, plausible-looking history.
        std::fs::write(&path, &full[..full.len() - 5]).unwrap();
        assert!(load_points(&path).is_none());

        std::fs::write(&path, &full[..full.len() - 1]).unwrap();
        assert!(load_points(&path).is_none());
    }

    #[test]
    fn corrupted_points_body_fails_the_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let path = points_path(dir.path(), Path::new("/repos/example"));
        save_points(
            &path,
            [3u8; OID_LEN],
            &[([4u8; OID_LEN], point(1, 1, 1, false))],
        )
        .unwrap();

        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();

        assert!(load_points(&path).is_none());
    }

    #[test]
    fn unknown_version_and_magic_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = points_path(dir.path(), Path::new("/repos/example"));
        save_points(
            &path,
            [5u8; OID_LEN],
            &[([6u8; OID_LEN], point(1, 0, 0, false))],
        )
        .unwrap();

        let bytes = std::fs::read(&path).unwrap();

        let mut bumped = bytes.clone();
        bumped[4..8].copy_from_slice(&(POINTS_VERSION + 1).to_le_bytes());
        std::fs::write(&path, &bumped).unwrap();
        assert!(load_points(&path).is_none());

        let mut wrong_magic = bytes;
        wrong_magic[0..4].copy_from_slice(b"XXXX");
        std::fs::write(&path, &wrong_magic).unwrap();
        assert!(load_points(&path).is_none());
    }

    #[test]
    fn chart_serves_only_the_exact_request() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Path::new("/repos/example");
        let tip = [11u8; OID_LEN];
        let buckets = vec![
            ChartBucket {
                start_seconds: 0,
                end_seconds: 3600,
                commits: 3,
                additions: 10,
                deletions: 4,
            },
            ChartBucket {
                start_seconds: 3600,
                end_seconds: 7200,
                commits: 0,
                additions: 0,
                deletions: 0,
            },
        ];
        let path = chart_path(dir.path(), repo, Some(BucketSize::Hour), false);
        save_chart(&path, tip, 42, BucketSize::Hour, false, 3, &buckets).unwrap();

        let hit = load_chart(&path, tip, 42, false).unwrap();
        assert_eq!(hit.bucket, BucketSize::Hour);
        assert_eq!(hit.total_commits, 3);
        assert_eq!(hit.buckets, buckets);

        // A different tip or exclusion set is a different aggregate and must
        // not be served from this file.
        assert!(load_chart(&path, [12u8; OID_LEN], 42, false).is_none());
        assert!(load_chart(&path, tip, 43, false).is_none());
        assert!(load_chart(&path, tip, 42, true).is_none());

        // Bucket size and merge flag live in the filename, so each variant
        // keeps its own file rather than evicting the others.
        let daily = chart_path(dir.path(), repo, Some(BucketSize::Day), false);
        let merges = chart_path(dir.path(), repo, Some(BucketSize::Hour), true);
        let auto = chart_path(dir.path(), repo, None, false);
        assert_ne!(path, daily);
        assert_ne!(path, merges);
        assert_ne!(path, auto);
        assert!(!daily.exists());
        assert!(!merges.exists());
        assert!(!auto.exists());
    }

    #[test]
    fn auto_bucket_chart_records_the_resolved_size() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Path::new("/repos/example");
        let tip = [21u8; OID_LEN];
        // Requested nothing, resolved to Month: the file has to carry that
        // forward or the next request cannot answer without a walk.
        let path = chart_path(dir.path(), repo, None, false);
        save_chart(&path, tip, 7, BucketSize::Month, false, 9, &[]).unwrap();

        let hit = load_chart(&path, tip, 7, false).unwrap();
        assert_eq!(hit.bucket, BucketSize::Month);
        assert_eq!(hit.total_commits, 9);
        assert!(hit.buckets.is_empty());
    }

    #[test]
    fn corrupted_chart_file_fails_the_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let repo = Path::new("/repos/example");
        let tip = [31u8; OID_LEN];
        let path = chart_path(dir.path(), repo, Some(BucketSize::Day), false);
        save_chart(
            &path,
            tip,
            1,
            BucketSize::Day,
            false,
            1,
            &[ChartBucket {
                start_seconds: 0,
                end_seconds: 86400,
                commits: 1,
                additions: 5,
                deletions: 2,
            }],
        )
        .unwrap();

        let mut bytes = std::fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();

        assert!(load_chart(&path, tip, 1, false).is_none());
    }

    #[test]
    fn repos_with_similar_paths_get_distinct_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = points_path(dir.path(), Path::new("/repos/app"));
        let b = points_path(dir.path(), Path::new("/repos/app2"));
        assert_ne!(a, b);

        // Windows paths share a drive prefix and separators; only the tail
        // differs, which is exactly where a prefix-only scheme would collide.
        let c = points_path(dir.path(), Path::new(r"C:\Users\a\Documents\GitHub\gitau"));
        let d = points_path(dir.path(), Path::new(r"C:\Users\a\Documents\GitHub\gitau2"));
        assert_ne!(c, d);
    }

    #[test]
    fn empty_points_file_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = points_path(dir.path(), Path::new("/repos/empty"));
        save_points(&path, [0u8; OID_LEN], &[]).unwrap();
        let loaded = load_points(&path).unwrap();
        assert!(loaded.records.is_empty());
        assert_eq!(loaded.tip, [0u8; OID_LEN]);
    }
}
