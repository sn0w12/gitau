use std::hint::black_box;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::AtomicBool;

use criterion::{Criterion, criterion_group, criterion_main};
use gix::bstr::BStr;
use gix::diff::Rewrites;
use gix::dir::walk::EmissionMode;
use gix::status::index_worktree::BuiltinSubmoduleStatus;
use gix::status::plumbing::index_as_worktree::traits::{CompareBlobs, FastEq, SubmoduleStatus};
use gix::status::plumbing::index_as_worktree_with_renames::{Sorting, VisitEntry};
use gix::status::tree_index::TrackRenames;

use git_backend::engines::gix::session::{GixSession, open_index, unrestricted_pathspec};
use git_backend::error::GitError;

/// libgit2-compatible rename settings, mirroring the status engine.
fn rewrites() -> Rewrites {
    Rewrites {
        copies: None,
        percentage: Some(0.5),
        limit: 1000,
        track_empty: false,
    }
}

/// A busy working tree: staged edits, unstaged edits, staged adds, untracked
/// files (including a collapsible directory), unstaged and staged renames,
/// and ignored files exercising the dirwalk. This is the shape the changes
/// panel loads after every mutation: the cache is generation-keyed, so each
/// stage/discard/commit pays a full cold status.
fn fixture_repo() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("bench-status");
        std::fs::create_dir_all(root.join("src")).unwrap();
        let repo = git2::Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Bench").unwrap();
        config.set_str("user.email", "bench@example.com").unwrap();
        config.set_str("core.autocrlf", "false").unwrap();
        drop(config);

        // 400 tracked files with unique content.
        for i in 0..400u32 {
            let content: String = (0..30).map(|j| format!("file {i} line {j}\n")).collect();
            std::fs::write(root.join(format!("src/file{i:03}.txt")), content).unwrap();
        }
        let mut index = repo.index().unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Bench", "bench@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "base", &tree, &[])
            .unwrap();

        // 60 staged modifications: index and worktree both move.
        for i in 0..60u32 {
            let content: String = (0..30).map(|j| format!("staged {i} line {j}\n")).collect();
            let path = format!("src/file{i:03}.txt");
            std::fs::write(root.join(&path), &content).unwrap();
            index.add_path(std::path::Path::new(&path)).unwrap();
        }
        index.write().unwrap();

        // 80 unstaged modifications: worktree only.
        for i in 60..140u32 {
            let content: String = (0..30)
                .map(|j| format!("worktree {i} line {j}\n"))
                .collect();
            std::fs::write(root.join(format!("src/file{i:03}.txt")), &content).unwrap();
        }

        // 15 staged additions.
        for i in 0..15u32 {
            let path = format!("src/added{i:02}.txt");
            std::fs::write(root.join(&path), format!("staged add {i}\n")).unwrap();
            index.add_path(std::path::Path::new(&path)).unwrap();
        }
        index.write().unwrap();

        // 25 loose untracked files plus a nested directory that collapses.
        for i in 0..25u32 {
            std::fs::write(
                root.join(format!("untracked{i:02}.txt")),
                format!("untracked {i}\n"),
            )
            .unwrap();
        }
        std::fs::create_dir_all(root.join("untracked-dir/nested")).unwrap();
        for i in 0..5u32 {
            std::fs::write(
                root.join(format!("untracked-dir/nested/f{i}.txt")),
                format!("nested {i}\n"),
            )
            .unwrap();
        }

        // 5 unstaged renames: byte-identical content, moved on disk only, so
        // rename detection pairs them exactly.
        for i in 0..5u32 {
            let old = format!("src/file{}.txt", 200 + i);
            let new = format!("src/renamed-unstaged{i}.txt");
            std::fs::rename(root.join(&old), root.join(&new)).unwrap();
        }

        // 3 staged renames: byte-identical content, moved on disk and in the
        // index.
        for i in 0..3u32 {
            let old = format!("src/file{}.txt", 210 + i);
            let new = format!("src/renamed-staged{i}.txt");
            std::fs::rename(root.join(&old), root.join(&new)).unwrap();
            index.remove_path(std::path::Path::new(&old)).unwrap();
            index.add_path(std::path::Path::new(&new)).unwrap();
        }
        index.write().unwrap();

        // Ignored noise so the dirwalk pays the ignore stack.
        std::fs::write(root.join(".gitignore"), "ignored*.log\n").unwrap();
        for i in 0..10u32 {
            std::fs::write(root.join(format!("ignored{i}.log")), "noise\n").unwrap();
        }

        std::mem::forget(outer);
        root
    })
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
}

fn session_for(root: &std::path::Path) -> GixSession {
    static S: OnceLock<GixSession> = OnceLock::new();
    S.get_or_init(|| GixSession::discover(root).unwrap())
        .clone()
}

fn bench_status(c: &mut Criterion) {
    let root = fixture_repo().clone();
    let backend = git_backend::Backend::new(git_backend::BackendConfig {
        watch_worktree: false,
        ..Default::default()
    });
    let opened = runtime().block_on(backend.open_repository(&root)).unwrap();
    let opts = git_backend::api::changes::StatusOptions::default();

    // Validate the fixture and prime the cache outside the measured region.
    let seeded = runtime()
        .block_on(backend.status(opened.id, opts, Default::default()))
        .unwrap();
    assert_eq!(seeded.staged_count(), 78, "fixture staged entries");
    assert_eq!(seeded.unstaged_count(), 116, "fixture worktree entries");

    c.bench_function("status/warm-cache-hit", |b| {
        b.iter(|| {
            let report =
                runtime().block_on(backend.status(opened.id, black_box(opts), Default::default()));
            black_box(report.unwrap().entries.len());
        })
    });

    // What the changes panel pays after each mutation: the generation-keyed
    // cache is empty, so the report is recomputed from scratch.
    c.bench_function("status/cold-recompute-194-entries", |b| {
        b.iter(|| {
            backend.clear_repository_caches(opened.id).unwrap();
            let report =
                runtime().block_on(backend.status(opened.id, black_box(opts), Default::default()));
            black_box(report.unwrap().entries.len());
        })
    });
}

fn bench_status_phases(c: &mut Criterion) {
    let root = fixture_repo().clone();
    let session = session_for(root.as_path());

    // Sweep thread_limit for the tracked-mod check to find the scaling curve.
    for tl in [Some(1), Some(2), Some(4), Some(8), Some(16), None] {
        c.bench_with_input(
            criterion::BenchmarkId::new("status/sweep-tl", format!("{:?}", tl)),
            &tl,
            |b, tl| {
                b.iter(|| {
                    let repo = session.handle();
                    let index = open_index(&repo).unwrap();
                    let options = gix::status::index_worktree::Options {
                        sorting: None,
                        dirwalk_options: None,
                        rewrites: None,
                        thread_limit: *tl,
                    };
                    let submodule =
                        BuiltinSubmoduleStatus::new(repo.clone().into_sync(), Default::default())
                            .unwrap();
                    let should_interrupt = AtomicBool::new(false);
                    let mut collector = CountCollector::default();
                    repo.index_worktree_status(
                        &index,
                        [] as [&gix::bstr::BStr; 0],
                        &mut collector,
                        FastEq,
                        submodule,
                        &mut gix::progress::Discard,
                        &should_interrupt,
                        options,
                    )
                    .unwrap();
                    black_box(collector.count);
                })
            },
        );
    }

    // Attribution: the staged side alone (HEAD -> index, with renames).
    c.bench_function("status/phases/staged-side", |b| {
        b.iter(|| {
            let repo = session.handle();
            let index = open_index(&repo).unwrap();
            let head_id = repo.head_id().unwrap();
            let commit = head_id.object().unwrap().try_into_commit().unwrap();
            let tree_id = commit.tree_id().unwrap().detach();
            let mut pathspec = unrestricted_pathspec(&repo).unwrap();
            let mut count = 0usize;
            repo.tree_index_status(
                &tree_id,
                &index,
                Some(&mut pathspec),
                TrackRenames::Given(rewrites()),
                |_change, _lhs, _rhs| -> std::result::Result<std::ops::ControlFlow<()>, GitError> {
                    count += 1;
                    Ok(std::ops::ControlFlow::Continue(()))
                },
            )
            .unwrap();
            assert_eq!(count, 78);
            black_box(count);
        })
    });

    // Attribution: the worktree side alone (index -> worktree, untracked
    // discovery, renames), mirroring the engine's options.
    c.bench_function("status/phases/worktree-side", |b| {
        b.iter(|| {
            let repo = session.handle();
            let index = open_index(&repo).unwrap();
            let mut dirwalk = repo.dirwalk_options().unwrap();
            dirwalk.set_emit_untracked(EmissionMode::Matching);
            dirwalk.set_emit_ignored(None);
            let options = gix::status::index_worktree::Options {
                sorting: Some(Sorting::ByPathCaseSensitive),
                dirwalk_options: Some(dirwalk),
                rewrites: Some(rewrites()),
                thread_limit: None,
            };
            let submodule =
                BuiltinSubmoduleStatus::new(repo.clone().into_sync(), Default::default()).unwrap();
            let should_interrupt = AtomicBool::new(false);
            let mut collector = CountCollector::default();
            repo.index_worktree_status(
                &index,
                [] as [&BStr; 0],
                &mut collector,
                FastEq,
                submodule,
                &mut gix::progress::Discard,
                &should_interrupt,
                options,
            )
            .unwrap();
            assert_eq!(collector.count, 116);
            black_box(collector.count);
        })
    });

    // Attribution: the tracked-modification check only, no dirwalk and no
    // renames. Whatever this leaves of the worktree-side cost is the
    // per-file lstat + stat-compare work on the index entries.
    c.bench_function("status/phases/tracked-mods-only", |b| {
        b.iter(|| {
            let repo = session.handle();
            let index = open_index(&repo).unwrap();
            let options = gix::status::index_worktree::Options {
                sorting: None,
                dirwalk_options: None,
                rewrites: None,
                thread_limit: None,
            };
            let submodule =
                BuiltinSubmoduleStatus::new(repo.clone().into_sync(), Default::default()).unwrap();
            let should_interrupt = AtomicBool::new(false);
            let mut collector = CountCollector::default();
            repo.index_worktree_status(
                &index,
                [] as [&BStr; 0],
                &mut collector,
                FastEq,
                submodule,
                &mut gix::progress::Discard,
                &should_interrupt,
                options,
            )
            .unwrap();
            // Without rewrites the rename sources surface as plain removals.
            assert_eq!(collector.count, 85);
            black_box(collector.count);
        })
    });

    // Attribution: full worktree side without rewrite tracking, isolating
    // what the rename tracker (including worktree-id hashing of untracked
    // content) adds on top.
    c.bench_function("status/phases/worktree-side-no-renames", |b| {
        b.iter(|| {
            let repo = session.handle();
            let index = open_index(&repo).unwrap();
            let mut dirwalk = repo.dirwalk_options().unwrap();
            dirwalk.set_emit_untracked(EmissionMode::Matching);
            dirwalk.set_emit_ignored(None);
            let options = gix::status::index_worktree::Options {
                sorting: Some(Sorting::ByPathCaseSensitive),
                dirwalk_options: Some(dirwalk),
                rewrites: None,
                thread_limit: None,
            };
            let submodule =
                BuiltinSubmoduleStatus::new(repo.clone().into_sync(), Default::default()).unwrap();
            let should_interrupt = AtomicBool::new(false);
            let mut collector = CountCollector::default();
            repo.index_worktree_status(
                &index,
                [] as [&BStr; 0],
                &mut collector,
                FastEq,
                submodule,
                &mut gix::progress::Discard,
                &should_interrupt,
                options,
            )
            .unwrap();
            // Without rewrites, the 5 rename sources surface as plain
            // removals (5 extra entries vs the full run).
            assert_eq!(collector.count, 121);
            black_box(collector.count);
        })
    });

    // Attribution: a bare dirwalk through the public API. Runs the same
    // exclude/attribute setup as the status path plus the full walk, but no
    // tracked-file modification checks.
    c.bench_function("status/phases/dirwalk-only", |b| {
        b.iter(|| {
            let repo = session.handle();
            let index = open_index(&repo).unwrap();
            let mut dirwalk = repo.dirwalk_options().unwrap();
            dirwalk.set_emit_untracked(EmissionMode::Matching);
            dirwalk.set_emit_ignored(None);
            let should_interrupt = AtomicBool::new(false);
            let mut delegate = gix::dir::walk::delegate::Collect::default();
            repo.dirwalk(
                &index,
                [] as [&BStr; 0],
                &should_interrupt,
                dirwalk,
                &mut delegate,
            )
            .unwrap();
            let entries = delegate.into_entries_by_path();
            // 30 untracked files, 5 rename sources surfaced as untracked, and
            // the .gitignore as a tracked-modified entry.
            assert_eq!(entries.len(), 36);
            black_box(entries.len());
        })
    });
}

#[derive(Default)]
struct CountCollector {
    count: usize,
}

impl<'index> VisitEntry<'index> for CountCollector {
    type ContentChange = <FastEq as CompareBlobs>::Output;
    type SubmoduleStatus = <BuiltinSubmoduleStatus as SubmoduleStatus>::Output;

    fn visit_entry(
        &mut self,
        entry: gix::status::plumbing::index_as_worktree_with_renames::Entry<
            'index,
            Self::ContentChange,
            Self::SubmoduleStatus,
        >,
    ) {
        use gix::dir::entry::{Kind, Status as DirStatus};
        use gix::status::plumbing::index_as_worktree::EntryStatus;
        use gix::status::plumbing::index_as_worktree_with_renames::Entry;
        match entry {
            Entry::Modification { status, .. } => match status {
                EntryStatus::Change(_) | EntryStatus::Conflict { .. } => self.count += 1,
                EntryStatus::NeedsUpdate(_) | EntryStatus::IntentToAdd => {}
            },
            Entry::DirectoryContents { entry, .. } => match (&entry.status, entry.disk_kind) {
                (DirStatus::Untracked, Some(Kind::Directory)) | (DirStatus::Untracked, _) => {
                    self.count += 1
                }
                _ => {}
            },
            Entry::Rewrite { .. } => self.count += 1,
        }
    }
}

criterion_group!(benches, bench_status, bench_status_phases);
criterion_main!(benches);
