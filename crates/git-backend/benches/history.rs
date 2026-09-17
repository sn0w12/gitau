use std::hint::black_box;
use std::path::PathBuf;
use std::sync::OnceLock;

use criterion::{Criterion, criterion_group, criterion_main};
use git_backend::api::history::{HistoryChartQuery, HistoryPageQuery};
use git_backend::{Backend, BackendConfig};

fn fixture_repo() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("bench-history");
        std::fs::create_dir_all(&root).unwrap();
        let repo = git2::Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Bench").unwrap();
        config.set_str("user.email", "bench@example.com").unwrap();
        drop(config);

        let mut index_holder = None;
        for i in 0..2000u32 {
            let content = format!("content for revision {i}\n");
            std::fs::write(root.join("file.txt"), &content).unwrap();
            let mut index = repo.index().unwrap();
            index.add_path(std::path::Path::new("file.txt")).unwrap();
            index.write().unwrap();
            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = git2::Signature::now("Bench", "bench@example.com").unwrap();
            let parents: Vec<git2::Commit> = match repo.head() {
                Ok(head) => vec![head.peel_to_commit().unwrap()],
                Err(_) => vec![],
            };
            let refs: Vec<&git2::Commit> = parents.iter().collect();
            let oid = repo
                .commit(
                    Some("HEAD"),
                    &sig,
                    &sig,
                    &format!("commit {i}"),
                    &tree,
                    refs.as_slice(),
                )
                .unwrap();
            index_holder = Some(oid);
        }
        black_box(index_holder);
        std::mem::forget(outer);
        root
    })
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
}

fn bench_history(c: &mut Criterion) {
    let root = fixture_repo().clone();
    let backend = Backend::new(BackendConfig {
        watch_worktree: false,
        ..Default::default()
    });
    let opened = runtime().block_on(backend.open_repository(&root)).unwrap();

    // Prime the cache so `page-100-of-2000` measures steady-state hits from
    // the first sample, not the initial miss.
    runtime()
        .block_on(backend.history_page(
            opened.id,
            HistoryPageQuery {
                limit: 100,
                ..Default::default()
            },
            Default::default(),
        ))
        .unwrap();

    c.bench_function("history/page-100-of-2000", |b| {
        b.iter(|| {
            let page = runtime().block_on(backend.history_page(
                opened.id,
                black_box(HistoryPageQuery {
                    limit: 100,
                    ..Default::default()
                }),
                Default::default(),
            ));
            assert_eq!(page.unwrap().commits.len(), 100);
        })
    });

    // Search scans the whole cached walk before paginating; every commit
    // matches "commit" so this isolates the filter itself. Cleared per
    // iteration because pages are generation-keyed cached values.
    c.bench_function("history/search-full-walk", |b| {
        b.iter(|| {
            backend.clear_repository_caches(opened.id).unwrap();
            let page = runtime().block_on(backend.history_page(
                opened.id,
                black_box(HistoryPageQuery {
                    limit: 100,
                    search: Some("commit 1".to_owned()),
                    ..Default::default()
                }),
                Default::default(),
            ));
            assert_eq!(page.unwrap().commits.len(), 100);
        })
    });

    c.bench_function("history/page-100-cold", |b| {
        b.iter(|| {
            backend.clear_repository_caches(opened.id).unwrap();
            let page = runtime().block_on(backend.history_page(
                opened.id,
                black_box(HistoryPageQuery {
                    limit: 100,
                    skip: 150,
                    ..Default::default()
                }),
                Default::default(),
            ));
            black_box(page.unwrap().commits.len());
        })
    });

    // Clears immutable history caches too: the honest worst case for a page
    // the process has never served.
    c.bench_function("history/page-100-trulycold", |b| {
        b.iter(|| {
            backend.clear_repository_history_caches(opened.id).unwrap();
            backend.clear_repository_caches(opened.id).unwrap();
            let page = runtime().block_on(backend.history_page(
                opened.id,
                black_box(HistoryPageQuery {
                    limit: 100,
                    skip: 150,
                    ..Default::default()
                }),
                Default::default(),
            ));
            black_box(page.unwrap().commits.len());
        })
    });

    // Aggregates the full 2000-commit walk with per-commit diff stats. Every
    // commit is new to the stats cache per iteration, so this is the full
    // revwalk plus 2000 tree diffs.
    c.bench_function("history/chart-cold", |b| {
        b.iter(|| {
            backend.clear_repository_history_caches(opened.id).unwrap();
            backend.clear_repository_caches(opened.id).unwrap();
            let chart = runtime().block_on(backend.history_chart(
                opened.id,
                black_box(HistoryChartQuery::default()),
                Default::default(),
            ));
            assert_eq!(chart.unwrap().total_commits, 2000);
        })
    });

    // Same work after the caches have settled: walk and stat points are all
    // hits, so this isolates aggregation.
    c.bench_function("history/chart-warm", |b| {
        b.iter(|| {
            let chart = runtime().block_on(backend.history_chart(
                opened.id,
                black_box(HistoryChartQuery::default()),
                Default::default(),
            ));
            assert_eq!(chart.unwrap().total_commits, 2000);
        })
    });

    // The chart after a page of history has been served: the walk and the
    // first 100 summaries are already cached, so only the remaining commits
    // pay for diff stats.
    c.bench_function("history/chart-after-page", |b| {
        b.iter(|| {
            backend.clear_repository_history_caches(opened.id).unwrap();
            backend.clear_repository_caches(opened.id).unwrap();
            runtime()
                .block_on(backend.history_page(
                    opened.id,
                    black_box(HistoryPageQuery {
                        limit: 100,
                        ..Default::default()
                    }),
                    Default::default(),
                ))
                .unwrap();
            let chart = runtime().block_on(backend.history_chart(
                opened.id,
                black_box(HistoryChartQuery::default()),
                Default::default(),
            ));
            assert_eq!(chart.unwrap().total_commits, 2000);
        })
    });
}

criterion_group!(benches, bench_history);
criterion_main!(benches);
