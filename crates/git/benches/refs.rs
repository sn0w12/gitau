use std::hint::black_box;
use std::path::PathBuf;
use std::sync::OnceLock;

use criterion::{Criterion, criterion_group, criterion_main};
use git::engines::git2::mutations::{list_branches, list_tags};
use git::{Backend, BackendConfig};
use git2::{Oid, Repository, Signature, Time};

const MAIN_COMMITS: usize = 800;
const BRANCHES: usize = 150;
const TAGS: usize = 100;
const BASE_SECONDS: i64 = 1_700_000_000;

fn commit(
    repo: &Repository,
    tree: &git2::Tree<'_>,
    message: &str,
    parent: Option<Oid>,
    seconds: i64,
) -> Oid {
    let sig = Signature::new("Bench", "bench@example.com", &Time::new(seconds, 0)).unwrap();
    let owned: Vec<git2::Commit<'_>> = parent
        .into_iter()
        .map(|oid| repo.find_commit(oid).unwrap())
        .collect();
    let parents: Vec<&git2::Commit<'_>> = owned.iter().collect();
    repo.commit(None, &sig, &sig, message, tree, &parents)
        .unwrap()
}

/// An 800-commit main line with 150 local branches whose upstream is 800
/// commits behind. Listing refs runs one `graph_ahead_behind` per tracking
/// branch, so the cost scales with branches times ancestry distance.
fn fixture_repo() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("bench-refs");
        std::fs::create_dir_all(&root).unwrap();
        let repo = Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Bench").unwrap();
        config.set_str("user.email", "bench@example.com").unwrap();
        drop(config);
        repo.remote("origin", "https://example.com/bench.git")
            .unwrap();

        let empty_tree = repo.treebuilder(None).unwrap().write().unwrap();
        let tree = repo.find_tree(empty_tree).unwrap();

        let mut main = Vec::with_capacity(MAIN_COMMITS);
        let mut parent: Option<Oid> = None;
        for i in 0..MAIN_COMMITS {
            let oid = commit(
                &repo,
                &tree,
                &format!("main {i}"),
                parent,
                BASE_SECONDS + i as i64,
            );
            main.push(oid);
            parent = Some(oid);
        }

        let base = main[0];
        let tip = *main.last().unwrap();
        let head = repo.find_commit(tip).unwrap();

        for branch in 0..BRANCHES {
            let name = format!("feature-{branch:03}");
            let mut local = repo.branch(&name, &head, false).unwrap();
            repo.reference(&format!("refs/remotes/origin/{name}"), base, true, "bench")
                .unwrap();
            local.set_upstream(Some(&format!("origin/{name}"))).unwrap();
        }

        repo.reference("refs/heads/main", tip, true, "bench")
            .unwrap();
        repo.set_head("refs/heads/main").unwrap();

        for tag in 0..TAGS {
            let target = main[(tag * (MAIN_COMMITS / TAGS)) % MAIN_COMMITS];
            let commit = repo.find_commit(target).unwrap();
            let sig = Signature::new(
                "Bench",
                "bench@example.com",
                &Time::new(BASE_SECONDS + tag as i64, 0),
            )
            .unwrap();
            repo.tag(
                &format!("v{tag:03}"),
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

fn bench_refs(c: &mut Criterion) {
    let root = fixture_repo().clone();
    let repo = Repository::discover(&root).unwrap();

    let branches = list_branches(&repo).unwrap();
    assert_eq!(branches.len(), BRANCHES + 1);
    assert_eq!(
        branches
            .iter()
            .filter(|branch| branch.upstream.is_some())
            .count(),
        BRANCHES
    );
    let tags = list_tags(&repo).unwrap();
    assert_eq!(tags.len(), TAGS);

    let backend = Backend::new(BackendConfig {
        watch_worktree: false,
        ..Default::default()
    });
    let opened = runtime().block_on(backend.open_repository(&root)).unwrap();

    let mut group = c.benchmark_group("refs");
    group.sample_size(10);

    group.bench_function("list-branches-150-tracking-800-behind", |b| {
        b.iter(|| {
            let repo = Repository::discover(black_box(&root)).unwrap();
            let branches = list_branches(black_box(&repo)).unwrap();
            assert_eq!(branches.len(), BRANCHES + 1);
            black_box(branches.len())
        })
    });

    group.bench_function("list-tags-100-annotated", |b| {
        b.iter(|| {
            let repo = Repository::discover(black_box(&root)).unwrap();
            let tags = list_tags(black_box(&repo)).unwrap();
            assert_eq!(tags.len(), TAGS);
            black_box(tags.len())
        })
    });

    group.bench_function("list-branches-and-tags-backend", |b| {
        b.iter(|| {
            let listing = runtime()
                .block_on(backend.list_branches_and_tags(opened.id, Default::default()))
                .unwrap();
            assert_eq!(listing.branches.len(), BRANCHES + 1);
            assert_eq!(
                listing
                    .branches
                    .iter()
                    .filter(|branch| branch.upstream.is_some())
                    .count(),
                BRANCHES
            );
            assert_eq!(listing.tags.len(), TAGS);
            black_box(listing.branches.len() + listing.tags.len())
        })
    });

    group.finish();
}

criterion_group!(benches, bench_refs);
criterion_main!(benches);
