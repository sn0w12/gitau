use std::hint::black_box;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

use criterion::{Criterion, criterion_group, criterion_main};
use git_backend::api::queries::DiffRequest;
use git_backend::domain::{Generation, ObjectId, OperationId, RepoId, RevisionSpec, SnapshotId};
use git_backend::streaming::model::{DiffComparison, DiffEvent};
use git_backend::streaming::pipeline::{self, DiffJob};
use git_backend::streaming::store::DiffOperation;

fn fixture_repo() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("bench-diff");
        std::fs::create_dir_all(&root).unwrap();
        let repo = git2::Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Bench").unwrap();
        config.set_str("user.email", "bench@example.com").unwrap();
        config.set_str("core.autocrlf", "false").unwrap();
        drop(config);

        let content: String = (0..120).map(|j| format!("line {j} of file\n")).collect();
        let mut files: Vec<(String, &str)> = Vec::new();
        for i in 0..60u32 {
            files.push((format!("src/file{i}.txt"), content.as_str()));
        }
        let refs: Vec<(&str, &str)> = files.iter().map(|(p, c)| (p.as_str(), *c)).collect();

        for (path, data) in &refs {
            if let Some(parent) = root.join(path).parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(root.join(path), data).unwrap();
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

        let changed: String = (0..120).map(|j| format!("REWRITTEN line {j}\n")).collect();
        for i in 0..60u32 {
            std::fs::write(root.join(format!("src/file{i}.txt")), &changed).unwrap();
        }
        std::mem::forget(outer);
        root
    })
}

fn session_for(root: &std::path::Path) -> git_backend::engines::gix::GixSession {
    use std::sync::OnceLock as OL;
    static S: OL<git_backend::engines::gix::GixSession> = OL::new();
    S.get_or_init(|| git_backend::engines::gix::GixSession::discover(root).unwrap())
        .clone()
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| tokio::runtime::Runtime::new().unwrap())
}

fn run_full_diff(root: &std::path::Path) -> (u64, u64) {
    let op = Arc::new(DiffOperation::new(RepoId(1), OperationId(1), Generation(1)));
    let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
    let job = DiffJob::new(
        root.to_path_buf(),
        DiffRequest::default(),
        OperationId(1),
        SnapshotId(0),
        Generation(1),
        session_for(root),
    );
    // The pipeline runs on the shared blocking pool like production does;
    // only the diff work itself belongs inside the measurement.
    let op_for_job = op.clone();
    let total = runtime().block_on(async {
        let handle = tokio::task::spawn_blocking(move || pipeline::run(job, op_for_job, tx));
        let mut rows = 0u64;
        while let Some(event) = rx.recv().await {
            if let DiffEvent::Completed { total_rows, .. } = event {
                rows = total_rows;
            }
        }
        handle.await.unwrap();
        rows
    });
    let (stored, _, _) = op.totals();
    (total.max(stored), stored)
}

/// A large, deep tree (20 dirs x 50 files, unique content) with a commit that
/// touches 5 files. This is the shape where commit-diff enumeration strategy
/// matters: identical subtrees dominate the tree, so a diff that cannot skip
/// them pays for the whole repository on every commit click.
fn commit_fixture_repo() -> &'static PathBuf {
    static FIXTURE: OnceLock<PathBuf> = OnceLock::new();
    FIXTURE.get_or_init(|| {
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("bench-commit-diff");
        std::fs::create_dir_all(&root).unwrap();
        let repo = git2::Repository::init(&root).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Bench").unwrap();
        config.set_str("user.email", "bench@example.com").unwrap();
        config.set_str("core.autocrlf", "false").unwrap();
        drop(config);
        let content: String = (0..40)
            .map(|j| format!("line {j} unique content\n"))
            .collect();
        for d in 0..20u32 {
            let dir = root.join(format!("dir{d:02}"));
            std::fs::create_dir_all(&dir).unwrap();
            for i in 0..50u32 {
                let file_content: String = (0..40)
                    .map(|j| format!("dir {d} file {i} line {j}\n"))
                    .collect();
                std::fs::write(dir.join(format!("file{i:03}.txt")), &file_content).unwrap();
            }
        }
        let _ = content;
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

        // The measured commit: 5 files change, 999 stay identical.
        for i in 0..5u32 {
            let changed: String = (0..40).map(|j| format!("EDITED line {j}\n")).collect();
            std::fs::write(root.join(format!("dir00/file{i:03}.txt")), &changed).unwrap();
        }
        let mut index = repo.index().unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "edit 5 files", &tree, &[&parent])
            .unwrap();

        std::mem::forget(outer);
        root
    })
}

fn bench_diff_stream(c: &mut Criterion) {
    let root = fixture_repo().clone();

    c.bench_function("diff/full-stream-60-files-x120-lines", |b| {
        b.iter(|| {
            let (reported, stored) = black_box(run_full_diff(black_box(root.as_path())));
            assert_eq!(reported, stored);
            assert!(reported > 60 * 120);
        })
    });

    // The perceived-latency metric: open -> first Chunk event. Everything
    // before it renders as an empty pane; everything after streams in.
    c.bench_function("diff/time-to-first-chunk", |b| {
        b.iter(|| {
            let op = Arc::new(DiffOperation::new(RepoId(3), OperationId(3), Generation(1)));
            let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
            let job = DiffJob::new(
                root.to_path_buf(),
                DiffRequest::default(),
                OperationId(3),
                SnapshotId(0),
                Generation(1),
                session_for(root.as_path()),
            );
            let op_for_job = op.clone();
            let got_chunk = runtime().block_on(async {
                let handle =
                    tokio::task::spawn_blocking(move || pipeline::run(job, op_for_job, tx));
                let mut first_rows = 0usize;
                while let Some(event) = rx.recv().await {
                    if let DiffEvent::Chunk { rows, .. } = event {
                        first_rows = rows.len();
                        break;
                    }
                }
                // Stop the clock at the first chunk: cancel so workers exit
                // after their current section, then drain the terminal events
                // awaiting the job thread.
                op.cancel();
                while rx.recv().await.is_some() {}
                handle.await.unwrap();
                first_rows
            });
            assert!(got_chunk > 0);
        })
    });

    // Attribution: candidate discovery alone (gix status + merge, no rows).
    c.bench_function("diff/enumerate-working-tree", |b| {
        let session = session_for(root.as_path());
        let blobs = git_backend::engines::gix::diff::new_blob_cache();
        b.iter(|| {
            let plans = git_backend::engines::gix::diff::enumerate(
                &session,
                &DiffRequest::default(),
                &blobs,
            )
            .unwrap();
            assert_eq!(plans.len(), 60);
        })
    });

    // Attribution: one section's full materialization (blob read, worktree
    // read, imara diff, row construction).
    c.bench_function("diff/materialize-one-section", |b| {
        let session = session_for(root.as_path());
        let request = DiffRequest::default();
        let blobs = git_backend::engines::gix::diff::new_blob_cache();
        let plan = git_backend::engines::gix::diff::enumerate(&session, &request, &blobs).unwrap();
        assert_eq!(plan.len(), 60);
        b.iter(|| {
            let rows = git_backend::engines::gix::diff::materialize(
                &session,
                &request,
                black_box(&plan[0]),
                &blobs,
            )
            .unwrap()
            .rows;
            assert!(rows.len() > 120);
        })
    });

    c.bench_function("diff/range-read-1000-rows", |b| {
        // The producer runs once outside the measured region; this benchmark
        // isolates storage-backed range reads (two 500-row pages).
        let op = Arc::new(DiffOperation::new(RepoId(1), OperationId(2), Generation(1)));
        let (tx, rx) = tokio::sync::mpsc::channel::<DiffEvent>(4096);
        let job = DiffJob::new(
            root.to_path_buf(),
            DiffRequest::default(),
            OperationId(2),
            SnapshotId(0),
            Generation(1),
            session_for(root.as_path()),
        );
        pipeline::run(job, op.clone(), tx);
        drop(rx);
        assert_eq!(
            op.totals().0,
            run_full_diff(root.as_path()).1,
            "fixture rows"
        );

        b.iter(|| {
            let mut cursor = 0u64;
            let mut fetched = 0usize;
            for _ in 0..2 {
                let range = op.read_range(cursor, 500).unwrap();
                cursor = range.next_cursor;
                fetched += range.rows.len();
                if !range.has_more {
                    break;
                }
            }
            black_box(fetched);
        })
    });

    // Commit-diff enumeration on a 1000-file tree where the commit touches
    // 5 files: the case that separates subtree-skipping strategies from
    // whole-tree flattening.
    let commit_root = commit_fixture_repo().clone();
    let commit_session = session_for(commit_root.as_path());
    let commit_oid = {
        let repo = git2::Repository::open(commit_root.as_path()).unwrap();
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        ObjectId::from_bytes(head.id().as_bytes()).unwrap()
    };
    let commit_request = DiffRequest {
        comparison: DiffComparison::CommitToParent {
            commit: RevisionSpec::from_oid(commit_oid),
        },
        ..DiffRequest::default()
    };
    let blobs = git_backend::engines::gix::diff::new_blob_cache();
    let expected =
        git_backend::engines::gix::diff::enumerate(&commit_session, &commit_request, &blobs)
            .unwrap();
    assert_eq!(expected.len(), 5, "fixture: 5 changed files");

    c.bench_function("diff/enumerate-commit-to-parent-5-of-1000", |b| {
        b.iter(|| {
            let blobs = git_backend::engines::gix::diff::new_blob_cache();
            let plans = git_backend::engines::gix::diff::enumerate(
                &commit_session,
                &commit_request,
                &blobs,
            )
            .unwrap();
            assert_eq!(plans.len(), 5);
        })
    });
}

criterion_group!(benches, bench_diff_stream);
criterion_main!(benches);
