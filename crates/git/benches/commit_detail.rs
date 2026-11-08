use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use criterion::{Criterion, criterion_group, criterion_main};
use git::api::history::CommitDetailQuery;
use git::domain::{ChangeKind, RevisionSpec};
use git::{Backend, BackendConfig};
use git2::{IndexAddOption, Repository, Signature};

const FILES: usize = 400;
const RENAMES: usize = 50;
const LINES: usize = 30;

fn content(tag: &str, file: usize) -> String {
    (0..LINES)
        .map(|line| format!("{tag} {file} line {line}\n"))
        .collect()
}

fn write_tree(index: &mut git2::Index) -> git2::Oid {
    index.add_all(["*"], IndexAddOption::DEFAULT, None).unwrap();
    index.write().unwrap();
    index.write_tree().unwrap()
}

/// A commit touching every one of 400 files, with 50 exact renames. Rename
/// detection runs `find_similar` over the whole delta list, which the existing
/// huge-commit bench disables.
fn fixture_repo() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("bench-commit-detail");
        std::fs::create_dir_all(root.join("src")).unwrap();
        let repo = Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Bench").unwrap();
        config.set_str("user.email", "bench@example.com").unwrap();
        config.set_str("core.autocrlf", "false").unwrap();
        drop(config);

        for file in 0..FILES {
            std::fs::write(
                root.join(format!("src/file{file:03}.txt")),
                content("base", file),
            )
            .unwrap();
        }
        let mut index = repo.index().unwrap();
        let tree_id = write_tree(&mut index);
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = Signature::now("Bench", "bench@example.com").unwrap();
        let base = repo
            .commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
            .unwrap();

        for file in 0..FILES {
            let old = format!("src/file{file:03}.txt");
            if file < RENAMES {
                let new = format!("src/renamed{file:03}.txt");
                std::fs::rename(root.join(&old), root.join(&new)).unwrap();
                index.remove_path(Path::new(&old)).unwrap();
            } else {
                std::fs::write(root.join(&old), content("edited", file)).unwrap();
            }
        }
        let tree_id = write_tree(&mut index);
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.find_commit(base).unwrap();
        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "edit 350, rename 50",
            &tree,
            &[&parent],
        )
        .unwrap();

        std::mem::forget(outer);
        root
    })
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
}

fn bench_commit_detail(c: &mut Criterion) {
    let root = fixture_repo().clone();
    let backend = Backend::new(BackendConfig {
        watch_worktree: false,
        ..Default::default()
    });
    let opened = runtime().block_on(backend.open_repository(&root)).unwrap();

    let mut group = c.benchmark_group("commit-detail/400-files-50-renamed");
    group.sample_size(20);

    for detect_renames in [true, false] {
        let query = CommitDetailQuery {
            revision: RevisionSpec::head(),
            detect_renames,
        };
        let expected_files = if detect_renames {
            FILES
        } else {
            FILES + RENAMES
        };

        group.bench_function(
            if detect_renames {
                "detect-renames"
            } else {
                "no-renames"
            },
            |b| {
                b.iter(|| {
                    backend.clear_repository_caches(opened.id).unwrap();
                    let detail = runtime()
                        .block_on(backend.commit_detail(
                            opened.id,
                            black_box(query.clone()),
                            Default::default(),
                        ))
                        .unwrap();
                    assert_eq!(detail.detail.files.len(), expected_files);
                    if detect_renames {
                        assert_eq!(
                            detail
                                .detail
                                .files
                                .iter()
                                .filter(|file| file.kind == ChangeKind::Renamed)
                                .count(),
                            RENAMES
                        );
                    }
                    black_box(detail.detail.total_additions)
                })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_commit_detail);
criterion_main!(benches);
