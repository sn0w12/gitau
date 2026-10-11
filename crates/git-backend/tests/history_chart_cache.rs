//! The history chart's on-disk cache must return exactly what a cold
//! computation returns, for every load path, and must never serve one
//! repository's data for another.

mod common;

use common::TestRepo;
use git_backend::api::history::HistoryChartQuery;
use git_backend::domain::history::{BucketSize, HistoryChart};
use git_backend::domain::ids::RepoId;
use git_backend::{Backend, BackendConfig};
use std::path::PathBuf;
use std::sync::Arc;

const DAY: i64 = 86_400;
/// 2024-03-15T00:00:00Z.
const BASE: i64 = 1_709_251_200;

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
}

fn backend(cache_dir: &std::path::Path) -> Backend {
    let backend = Backend::new(BackendConfig {
        watch_worktree: false,
        ..Default::default()
    });
    backend.initialize_history_stats_cache(cache_dir.to_path_buf());
    backend
}

fn chart(backend: &Backend, id: RepoId, query: HistoryChartQuery) -> Arc<HistoryChart> {
    runtime()
        .block_on(backend.history_chart(id, query, Default::default()))
        .unwrap()
}

/// Commits with an explicit timestamp so they land in known, distinct buckets.
/// `TestRepo::commit_all` stamps `now()`, which would put the whole fixture in
/// one bucket and hide the properties under test.
fn commit_at(fixture: &TestRepo, message: &str, seconds: i64, files: &[(&str, &str)]) -> git2::Oid {
    for (rel, content) in files {
        fixture.write(rel, content);
    }
    let mut index = fixture.repo.index().unwrap();
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .unwrap();
    index.write().unwrap();
    let tree = fixture.repo.find_tree(index.write_tree().unwrap()).unwrap();
    let sig = git2::Signature::new(
        "Test User",
        "test@example.com",
        &git2::Time::new(seconds, 0),
    )
    .unwrap();
    let parents: Vec<git2::Commit> = match fixture.repo.head() {
        Ok(head) => vec![head.peel_to_commit().unwrap()],
        Err(_) => vec![],
    };
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    fixture
        .repo
        .commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)
        .unwrap()
}

/// Four commits twenty days apart. The span lands in the quarter-day
/// auto-selection tier with room to spare on both sides, so adding a nearby
/// commit does not move the tier and change every bucket boundary. A fixture
/// near a tier edge would make bucket changes look like a cache bug.
fn build_repo(fixture: &TestRepo) {
    commit_at(fixture, "initial", BASE, &[("a.txt", "one\ntwo\nthree\n")]);
    commit_at(
        fixture,
        "second",
        BASE + 20 * DAY,
        &[("a.txt", "one\ntwo\nthree\nfour\n")],
    );
    commit_at(
        fixture,
        "third",
        BASE + 40 * DAY,
        &[("b.txt", "new file\ncontents\n")],
    );
    commit_at(fixture, "fourth", BASE + 60 * DAY, &[("a.txt", "one\n")]);
}

#[test]
fn disk_cache_matches_a_cold_computation_exactly() {
    let fixture = TestRepo::init("disk-cache-parity");
    build_repo(&fixture);

    let store = tempfile::tempdir().unwrap();
    let cache_dir = store.path().to_path_buf();

    let first = backend(&cache_dir);
    let opened = runtime()
        .block_on(first.open_repository(&fixture.root))
        .unwrap();

    // No cache anywhere: this is the reference result.
    let uncached = chart(&first, opened.id, HistoryChartQuery::default());

    // Same process, in-memory caches dropped, tip unchanged. This exercises
    // the aggregate file, which answers without walking at all.
    first.clear_repository_history_caches(opened.id).unwrap();
    first.clear_repository_caches(opened.id).unwrap();
    let from_disk = chart(&first, opened.id, HistoryChartQuery::default());

    assert_eq!(uncached.buckets, from_disk.buckets);
    assert_eq!(uncached.bucket_size, from_disk.bucket_size);
    assert_eq!(uncached.total_commits, from_disk.total_commits);
    assert_eq!(uncached.total_commits, 4);

    // A brand new backend over the same directory is the app-relaunch case.
    let second = backend(&cache_dir);
    let reopened = runtime()
        .block_on(second.open_repository(&fixture.root))
        .unwrap();
    let relaunched = chart(&second, reopened.id, HistoryChartQuery::default());

    assert_eq!(
        uncached.buckets, relaunched.buckets,
        "a relaunch must reproduce the cold buckets exactly"
    );
    assert_eq!(uncached.total_commits, relaunched.total_commits);
    assert_eq!(uncached.bucket_size, relaunched.bucket_size);
}

#[test]
fn the_points_file_serves_a_moved_tip_without_a_walk_wide_diff() {
    let fixture = TestRepo::init("disk-cache-points");
    build_repo(&fixture);

    let store = tempfile::tempdir().unwrap();
    let cache_dir = store.path().to_path_buf();

    // Populate the points file, then throw away the aggregate file so the
    // next chart has to walk and aggregate from per-commit stats instead.
    let first = backend(&cache_dir);
    let opened = runtime()
        .block_on(first.open_repository(&fixture.root))
        .unwrap();
    let before = chart(&first, opened.id, HistoryChartQuery::default());
    for entry in std::fs::read_dir(&cache_dir).unwrap().flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with("chart-") {
            std::fs::remove_file(entry.path()).unwrap();
        }
    }

    // Move the tip. The aggregate is gone, so every point has to come from
    // the persisted file rather than a fresh tree diff.
    commit_at(&fixture, "fifth", BASE + 62 * DAY, &[("c.txt", "new\n")]);
    first.clear_repository_history_caches(opened.id).unwrap();
    first.clear_repository_caches(opened.id).unwrap();

    let after = chart(&first, opened.id, HistoryChartQuery::default());
    assert_eq!(after.total_commits, before.total_commits + 1);

    // Every bucket that existed before the new commit must be byte-identical,
    // which is only true if the old commits were read back correctly.
    for old in &before.buckets {
        let matching = after
            .buckets
            .iter()
            .find(|new| new.start_seconds == old.start_seconds)
            .expect("bucket survives");
        if matching.end_seconds <= BASE + 60 * DAY {
            assert_eq!(old, matching, "bucket at {} changed", old.start_seconds);
        }
    }
}

#[test]
fn a_new_commit_extends_the_cached_chart_without_changing_older_buckets() {
    let fixture = TestRepo::init("disk-cache-incremental");
    build_repo(&fixture);

    let store = tempfile::tempdir().unwrap();
    let cache_dir = store.path().to_path_buf();

    let first = backend(&cache_dir);
    let opened = runtime()
        .block_on(first.open_repository(&fixture.root))
        .unwrap();
    let before = chart(&first, opened.id, HistoryChartQuery::default());

    commit_at(
        &fixture,
        "a much later commit",
        BASE + 65 * DAY,
        &[("d.txt", "late\n")],
    );

    let second = backend(&cache_dir);
    let reopened = runtime()
        .block_on(second.open_repository(&fixture.root))
        .unwrap();
    let after = chart(&second, reopened.id, HistoryChartQuery::default());

    assert_eq!(after.total_commits, before.total_commits + 1);
    // Buckets are chronological and the new commit is the newest, so every
    // bucket up to and including the old last one must be unchanged.
    let old_last = before.buckets.last().unwrap().start_seconds;
    for new in after.buckets.iter().filter(|b| b.start_seconds <= old_last) {
        let old = before
            .buckets
            .iter()
            .find(|b| b.start_seconds == new.start_seconds)
            .expect("bucket survives");
        assert_eq!(old, new, "bucket at {} changed", new.start_seconds);
    }
}

#[test]
fn cached_chart_is_not_served_across_repositories() {
    let first_repo = TestRepo::init("disk-cache-isolation-a");
    let second_repo = TestRepo::init("disk-cache-isolation-b");
    build_repo(&first_repo);
    second_repo.initial_commit(&[("a.txt", "only\n")]);

    let store = tempfile::tempdir().unwrap();
    let cache_dir = store.path().to_path_buf();

    let backend = backend(&cache_dir);
    let a = runtime()
        .block_on(backend.open_repository(&first_repo.root))
        .unwrap();
    let b = runtime()
        .block_on(backend.open_repository(&second_repo.root))
        .unwrap();

    let chart_a = chart(&backend, a.id, HistoryChartQuery::default());
    let chart_b = chart(&backend, b.id, HistoryChartQuery::default());

    assert_eq!(chart_a.total_commits, 4);
    assert_eq!(chart_b.total_commits, 1);
    assert_ne!(chart_a.buckets, chart_b.buckets);
}

#[test]
fn a_corrupt_cache_file_is_ignored_rather_than_trusted() {
    let fixture = TestRepo::init("disk-cache-corrupt");
    build_repo(&fixture);

    let store = tempfile::tempdir().unwrap();
    let cache_dir = store_dir_path(store.path());

    let first = backend(&cache_dir);
    let opened = runtime()
        .block_on(first.open_repository(&fixture.root))
        .unwrap();
    let reference = chart(&first, opened.id, HistoryChartQuery::default());

    for entry in std::fs::read_dir(&cache_dir).unwrap().flatten() {
        std::fs::write(entry.path(), b"garbage that is not a cache file").unwrap();
    }

    let second = backend(&cache_dir);
    let reopened = runtime()
        .block_on(second.open_repository(&fixture.root))
        .unwrap();
    let recovered = chart(&second, reopened.id, HistoryChartQuery::default());

    assert_eq!(reference.buckets, recovered.buckets);
    assert_eq!(reference.total_commits, recovered.total_commits);
}

#[test]
fn an_explicit_bucket_size_gets_its_own_cache_file() {
    let fixture = TestRepo::init("disk-cache-bucket-variants");
    build_repo(&fixture);

    let store = tempfile::tempdir().unwrap();
    let cache_dir = store.path().to_path_buf();

    let first = backend(&cache_dir);
    let opened = runtime()
        .block_on(first.open_repository(&fixture.root))
        .unwrap();

    let auto = chart(&first, opened.id, HistoryChartQuery::default());
    let weekly = chart(
        &first,
        opened.id,
        HistoryChartQuery {
            bucket: Some(BucketSize::Week),
            ..Default::default()
        },
    );
    assert_eq!(auto.bucket_size, BucketSize::QuarterDay);
    assert_eq!(weekly.bucket_size, BucketSize::Week);
    assert_ne!(auto.buckets, weekly.buckets);
    assert_eq!(auto.total_commits, weekly.total_commits);

    // Both variants survive a relaunch independently.
    let second = backend(&cache_dir);
    let reopened = runtime()
        .block_on(second.open_repository(&fixture.root))
        .unwrap();
    assert_eq!(
        auto.buckets,
        chart(&second, reopened.id, HistoryChartQuery::default()).buckets
    );
    assert_eq!(
        weekly.buckets,
        chart(
            &second,
            reopened.id,
            HistoryChartQuery {
                bucket: Some(BucketSize::Week),
                ..Default::default()
            }
        )
        .buckets
    );
}

/// A cache that is written but never read still produces correct charts, so
/// correctness assertions cannot catch it. This pins the write path to the
/// lookup path: the file has to land under the name the next request asks for,
/// which is the request's bucket and not the resolved one.
#[test]
fn charts_are_written_under_the_name_the_next_request_looks_up() {
    let fixture = TestRepo::init("disk-cache-naming");
    build_repo(&fixture);

    let store = tempfile::tempdir().unwrap();
    let cache_dir = store.path().to_path_buf();

    let first = backend(&cache_dir);
    let opened = runtime()
        .block_on(first.open_repository(&fixture.root))
        .unwrap();
    chart(&first, opened.id, HistoryChartQuery::default());
    chart(
        &first,
        opened.id,
        HistoryChartQuery {
            bucket: Some(BucketSize::Week),
            ..Default::default()
        },
    );

    let mut names: Vec<String> = std::fs::read_dir(&cache_dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    println!("cache files: {names:?}");

    assert!(
        names.iter().any(|name| name.contains("-auto-0.bin")),
        "auto-selection request must persist to the auto path: {names:?}"
    );
    assert!(
        names.iter().any(|name| name.contains("-06-0.bin")),
        "explicit week request must persist to the week path: {names:?}"
    );
    assert_eq!(
        names
            .iter()
            .filter(|name| name.starts_with("points-"))
            .count(),
        1
    );
}

#[test]
fn untouched_stat_files_age_out_and_fresh_ones_survive() {
    use std::time::{Duration, SystemTime};

    let fixture = TestRepo::init("disk-cache-prune");
    build_repo(&fixture);

    let store = tempfile::tempdir().unwrap();
    let cache_dir = store.path().join("history-stats");

    let backend = backend(&cache_dir);
    let opened = runtime()
        .block_on(backend.open_repository(&fixture.root))
        .unwrap();
    chart(&backend, opened.id, HistoryChartQuery::default());

    let files: Vec<std::path::PathBuf> = std::fs::read_dir(&cache_dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.path())
        .collect();
    assert!(!files.is_empty(), "charting should have written stat files");

    // Nothing is old yet.
    assert_eq!(
        backend.prune_history_stats_cache(git_backend::HISTORY_STATS_PRUNE_AFTER),
        0
    );

    // Age one file past the horizon, then confirm only that one goes.
    let stale =
        SystemTime::now() - git_backend::HISTORY_STATS_PRUNE_AFTER - Duration::from_secs(60);
    let target = files[0].clone();
    for path in &files {
        std::fs::OpenOptions::new()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(if path == &target {
                SystemTime::now()
            } else {
                stale
            })
            .unwrap();
    }

    assert_eq!(
        backend.prune_history_stats_cache(git_backend::HISTORY_STATS_PRUNE_AFTER),
        files.len() - 1
    );
    assert!(target.exists());
    assert!(!files[1..].iter().any(|path| path.exists()));
}

fn store_dir_path(base: &std::path::Path) -> PathBuf {
    base.join("history-stats")
}
