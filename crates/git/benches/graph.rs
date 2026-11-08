use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use git::api::graph::GraphQuery;
use git::domain::{Generation, OperationId, RepoId, SnapshotId};
use git::engines::git2::graph::LaneTracker;
use git::runtime::history_cache::HistoryCache;
use git::streaming::graph::GraphOperation;
use git::streaming::graph_pipeline::{self, GraphJob};
use git::streaming::model::GraphEvent;
use git2::{Oid, Repository, Signature, Time};

const MAIN_COMMITS: usize = 2_000;
const BRANCHES: usize = 120;
const BRANCH_DEPTH: usize = 40;
const BASE_SECONDS: i64 = 1_700_000_000;

fn commit(
    repo: &Repository,
    tree: &git2::Tree<'_>,
    message: &str,
    parents: &[Oid],
    seconds: i64,
) -> Oid {
    let sig = Signature::new("Bench", "bench@example.com", &Time::new(seconds, 0)).unwrap();
    let owned: Vec<git2::Commit<'_>> = parents
        .iter()
        .map(|oid| repo.find_commit(*oid).unwrap())
        .collect();
    let parents: Vec<&git2::Commit<'_>> = owned.iter().collect();
    repo.commit(None, &sig, &sig, message, tree, &parents)
        .unwrap()
}

/// A wide history: a 2000-commit main line, 120 branches of 40 commits each,
/// and a merge with every branch tip as a parent. The walk keeps roughly 120
/// lanes occupied while it drains the branch chains, which is the shape the
/// lane tracker pays for.
fn fixture_repo() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("bench-graph");
        std::fs::create_dir_all(&root).unwrap();
        let repo = Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Bench").unwrap();
        config.set_str("user.email", "bench@example.com").unwrap();
        drop(config);

        let empty_tree = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(empty_tree).unwrap();

        let mut main = Vec::with_capacity(MAIN_COMMITS);
        let mut parent: Option<Oid> = None;
        for i in 0..MAIN_COMMITS {
            let parents: Vec<Oid> = parent.into_iter().collect();
            let oid = commit(
                &repo,
                &tree,
                &format!("main {i}"),
                &parents,
                BASE_SECONDS + i as i64,
            );
            main.push(oid);
            parent = Some(oid);
        }

        let base = main[0];
        let mut branch_tips = Vec::with_capacity(BRANCHES);
        for branch in 0..BRANCHES {
            let mut tip = base;
            for depth in 0..BRANCH_DEPTH {
                let seconds =
                    BASE_SECONDS + MAIN_COMMITS as i64 + 1 + (depth * BRANCHES + branch) as i64;
                tip = commit(
                    &repo,
                    &tree,
                    &format!("branch {branch} depth {depth}"),
                    &[tip],
                    seconds,
                );
            }
            branch_tips.push(tip);
            repo.branch(
                &format!("branch-{branch:03}"),
                &repo.find_commit(tip).unwrap(),
                false,
            )
            .unwrap();
        }

        let mut merge_parents = vec![*main.last().unwrap()];
        merge_parents.extend(branch_tips.iter().copied());
        let merge_seconds =
            BASE_SECONDS + MAIN_COMMITS as i64 + (BRANCHES * BRANCH_DEPTH) as i64 + 10;
        let merge = commit(
            &repo,
            &tree,
            "merge every branch",
            &merge_parents,
            merge_seconds,
        );

        repo.reference("refs/heads/main", merge, true, "bench")
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();

        for tag in 0..50usize {
            let target = main[tag * (MAIN_COMMITS / 50)];
            let commit = repo.find_commit(target).unwrap();
            let sig = Signature::new(
                "Bench",
                "bench@example.com",
                &Time::new(BASE_SECONDS + tag as i64, 0),
            )
            .unwrap();
            repo.tag(
                &format!("tag-{tag:02}"),
                commit.as_object(),
                &sig,
                &format!("tag {tag}"),
                false,
            )
            .unwrap();
        }

        std::mem::forget(outer);
        root
    })
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
}

fn run_full_graph(root: &Path, cache: Arc<Mutex<HistoryCache>>) -> (u64, u64) {
    let op = Arc::new(GraphOperation::new(
        RepoId(1),
        OperationId(1),
        Generation(1),
    ));
    let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
    let job = GraphJob::new(
        root.to_path_buf(),
        GraphQuery::default(),
        OperationId(1),
        SnapshotId(0),
        Generation(1),
        cache,
    );
    let op_for_job = op.clone();
    let total = runtime().block_on(async {
        let handle = tokio::task::spawn_blocking(move || graph_pipeline::run(job, op_for_job, tx));
        let mut total = 0u64;
        while let Some(event) = rx.recv().await {
            if let GraphEvent::Completed { total_rows, .. } = event {
                total = total_rows;
            }
        }
        handle.await.unwrap();
        total
    });
    (total, op.row_count())
}

fn time_to_first_chunk(root: &Path, cache: Arc<Mutex<HistoryCache>>) -> usize {
    let op = Arc::new(GraphOperation::new(
        RepoId(2),
        OperationId(2),
        Generation(1),
    ));
    let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
    let job = GraphJob::new(
        root.to_path_buf(),
        GraphQuery::default(),
        OperationId(2),
        SnapshotId(0),
        Generation(1),
        cache,
    );
    let op_for_job = op.clone();
    runtime().block_on(async {
        let handle = tokio::task::spawn_blocking(move || graph_pipeline::run(job, op_for_job, tx));
        let mut rows = 0usize;
        while let Some(event) = rx.recv().await {
            if let GraphEvent::Chunk { rows: chunk, .. } = event {
                rows = chunk.len();
                break;
            }
        }
        op.cancel();
        while rx.recv().await.is_some() {}
        handle.await.unwrap();
        rows
    })
}

/// Steps `width` lanes for `rows` rows, keeping every lane occupied. Each step
/// scans the whole column list, which is the per-row cost the graph pays.
fn lane_steps(width: usize, rows: usize) -> u64 {
    let mut tracker = LaneTracker::<u64>::new();
    let parents: Vec<u64> = (1..=width as u64).collect();
    let mut pending = parents.clone();
    let mut edges = tracker.step(0, &parents).edges.len() as u64;
    for _ in 0..rows {
        for slot in &mut pending {
            let current = *slot;
            let next = current + width as u64;
            *slot = next;
            edges += tracker.step(current, &[next]).edges.len() as u64;
        }
    }
    edges
}

fn bench_graph(c: &mut Criterion) {
    let root = fixture_repo().as_path();
    let expected = (MAIN_COMMITS + BRANCHES * BRANCH_DEPTH + 1) as u64;

    let mut group = c.benchmark_group("graph");
    group.sample_size(10);

    group.bench_function("stream-cold-walk", |b| {
        b.iter(|| {
            let cache = Arc::new(Mutex::new(HistoryCache::default()));
            let (reported, stored) = run_full_graph(black_box(root), cache);
            assert_eq!(reported, expected);
            assert_eq!(stored, expected);
            black_box(reported)
        })
    });

    let warm_cache = Arc::new(Mutex::new(HistoryCache::default()));
    let (warm_rows, _) = run_full_graph(root, warm_cache.clone());
    assert_eq!(warm_rows, expected);

    group.bench_function("stream-warm-walk", |b| {
        b.iter(|| {
            let (reported, stored) = run_full_graph(root, warm_cache.clone());
            assert_eq!(reported, expected);
            assert_eq!(stored, expected);
            black_box(reported)
        })
    });

    group.bench_function("time-to-first-chunk-cold-walk", |b| {
        b.iter(|| {
            let cache = Arc::new(Mutex::new(HistoryCache::default()));
            let rows = time_to_first_chunk(root, cache);
            assert!(rows > 0);
            black_box(rows)
        })
    });

    group.finish();

    let mut lanes = c.benchmark_group("graph/lane-tracker");
    lanes.sample_size(20);
    for width in [1usize, 8, 32, 128, 512] {
        lanes.bench_with_input(BenchmarkId::new("step", width), &width, |b, &width| {
            b.iter(|| black_box(lane_steps(width, 100)));
        });
    }
    lanes.finish();
}

criterion_group!(benches, bench_graph);
criterion_main!(benches);
