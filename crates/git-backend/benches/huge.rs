//! History stats against a single very large commit: 400 files, ~50k
//! additions, ~25k deletions. The synthetic `history` fixture commits one
//! one-line file per revision and never exercises a wide, heavy diff.
//!
//! `cold` clears the in-memory caches so the first `limit=1` page (HEAD, the
//! huge commit) recomputes every file's line count. `warm` is the cache floor.

use std::path::PathBuf;
use std::sync::OnceLock;

use criterion::{Criterion, criterion_group, criterion_main};
use git_backend::api::history::{CommitDetailQuery, HistoryPageQuery};
use git_backend::domain::RevisionSpec;
use git_backend::{Backend, BackendConfig};
use git2::{Repository, Signature};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn build_fixture(root: &std::path::Path, files: usize, base_lines: usize, add: usize, del: usize) {
    std::fs::create_dir_all(root).unwrap();
    let repo = Repository::init(root).unwrap();
    {
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "Bench").unwrap();
        cfg.set_str("user.email", "bench@example.com").unwrap();
    }
    let sig = Signature::now("Bench", "bench@example.com").unwrap();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);

    let mut base: Vec<Vec<String>> = Vec::with_capacity(files);
    for f in 0..files {
        let lines: Vec<String> = (0..base_lines)
            .map(|i| {
                let n = (rng.next() % 1_000_000) as usize;
                format!("    base line_{f}_{i} value_{n} = compute({n});")
            })
            .collect();
        base.push(lines);
    }

    let write_tree = |repo: &Repository, contents: &[String]| {
        let mut tb = repo.treebuilder(None).unwrap();
        for (f, content) in contents.iter().enumerate() {
            let oid = repo.blob(content.as_bytes()).unwrap();
            tb.insert(format!("file_{f:04}.rs"), oid, 0o100644).unwrap();
        }
        tb.write().unwrap()
    };

    let base_contents: Vec<String> = base.iter().map(|l| l.join("\n") + "\n").collect();
    let base_tree = repo.find_tree(write_tree(&repo, &base_contents)).unwrap();
    let base_commit = repo
        .commit(Some("HEAD"), &sig, &sig, "base", &base_tree, &[])
        .unwrap();

    let huge_contents: Vec<String> = base
        .iter()
        .map(|lines| {
            let mut edited = lines.clone();
            for _ in 0..del {
                let at = (rng.next() % edited.len() as u64) as usize;
                edited.remove(at);
            }
            for _ in 0..add {
                let at = (rng.next() % (edited.len() as u64 + 1)) as usize;
                let n = (rng.next() % 1_000_000) as usize;
                edited.insert(at, format!("    new line value_{n} = compute({n});"));
            }
            edited.join("\n") + "\n"
        })
        .collect();
    let huge_tree = repo.find_tree(write_tree(&repo, &huge_contents)).unwrap();
    let parent = repo.find_commit(base_commit).unwrap();
    repo.commit(Some("HEAD"), &sig, &sig, "huge", &huge_tree, &[&parent])
        .unwrap();
}

fn fixture_repo() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("bench-huge");
        build_fixture(&root, 400, 800, 125, 62);
        std::mem::forget(outer);
        root
    })
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
}

fn bench_huge(c: &mut Criterion) {
    let root = fixture_repo().clone();
    let backend = Backend::new(BackendConfig {
        watch_worktree: false,
        ..Default::default()
    });
    let opened = runtime().block_on(backend.open_repository(&root)).unwrap();

    let query = HistoryPageQuery {
        limit: 1,
        ..Default::default()
    };

    c.bench_function("huge/history-page-cold", |b| {
        b.iter(|| {
            backend.clear_repository_caches(opened.id).unwrap();
            backend.clear_repository_history_caches(opened.id).unwrap();
            let page = runtime().block_on(backend.history_page(
                opened.id,
                query.clone(),
                Default::default(),
            ));
            let page = page.unwrap();
            assert_eq!(page.commits[0].files_changed, 400);
            assert_eq!(page.commits[0].additions, 50_000);
            assert_eq!(page.commits[0].deletions, 24_800);
        })
    });

    runtime()
        .block_on(backend.history_page(opened.id, query.clone(), Default::default()))
        .unwrap();
    c.bench_function("huge/history-page-warm", |b| {
        b.iter(|| {
            let page = runtime().block_on(backend.history_page(
                opened.id,
                query.clone(),
                Default::default(),
            ));
            assert_eq!(page.unwrap().commits.len(), 1);
        })
    });

    let detail_query = CommitDetailQuery {
        revision: RevisionSpec::parse("HEAD").unwrap(),
        detect_renames: false,
    };
    c.bench_function("huge/commit-detail-cold", |b| {
        b.iter(|| {
            backend.clear_repository_caches(opened.id).unwrap();
            backend.clear_repository_history_caches(opened.id).unwrap();
            let detail = runtime().block_on(backend.commit_detail(
                opened.id,
                detail_query.clone(),
                Default::default(),
            ));
            assert_eq!(detail.unwrap().detail.files.len(), 400);
        })
    });
}

criterion_group!(benches, bench_huge);
criterion_main!(benches);
