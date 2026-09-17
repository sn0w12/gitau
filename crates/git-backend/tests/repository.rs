mod common;

use common::TestRepo;
use git_backend::api::changes::StatusOptions;
use git_backend::api::history::HistoryPageQuery;
use git_backend::domain::{ChangeKind, ChangeSide, Generation};
use git_backend::{Backend, BackendConfig};

#[tokio::test]
async fn open_repository_reports_head_and_generation() {
    let repo = TestRepo::init("open-basic");
    repo.initial_commit(&[("README.md", "# hello")]);

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();

    let branch = opened.snapshot.head.branch().unwrap_or_default().to_owned();
    assert!(
        branch == "master" || branch == "main",
        "unexpected default branch {branch}"
    );
    assert!(matches!(
        opened.snapshot.head,
        git_backend::domain::HeadState::Attached { .. }
    ));
    assert_eq!(opened.snapshot.generation, Generation(1));
    assert!(!opened.snapshot.git_dir.as_os_str().is_empty());
}

#[tokio::test]
async fn open_repository_from_subdirectory_deduplicates() {
    let repo = TestRepo::init("dedupe");
    repo.initial_commit(&[("src/lib.rs", "pub fn f() {}")]);
    let nested = repo.root.join("src");

    let backend = Backend::new(BackendConfig::default());
    let first = backend.open_repository(&repo.root).await.unwrap();
    let second = backend.open_repository(&nested).await.unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(backend.open_repository_count(), 1);
}

#[tokio::test]
async fn open_rejects_non_repositories() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Backend::new(BackendConfig::default());
    let err = backend.open_repository(dir.path()).await.unwrap_err();
    assert_eq!(err.code(), "notARepository");
}

#[tokio::test]
async fn unborn_head_is_reported() {
    let repo = TestRepo::init("unborn");
    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();
    assert!(opened.snapshot.head.is_unborn());
}

#[tokio::test]
async fn mutations_bump_generation_and_invalidate_status_cache() {
    let repo = TestRepo::init("generations");
    repo.initial_commit(&[("a.txt", "one\n")]);
    repo.write("b.txt", "new file\n");

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();
    let opts = StatusOptions::default();

    let before = backend
        .status(opened.id, opts, Default::default())
        .await
        .unwrap();
    assert_eq!(before.unstaged_count(), 1);

    let cached_again = backend
        .status(opened.id, opts, Default::default())
        .await
        .unwrap();
    assert_eq!(cached_again.snapshot_id, before.snapshot_id);

    backend
        .stage_paths(
            opened.id,
            git_backend::api::changes::StageRequest {
                paths: vec![],
                all: true,
            },
            Some(opened.snapshot.generation),
        )
        .await
        .unwrap();

    let after = backend
        .status(opened.id, opts, Default::default())
        .await
        .unwrap();
    assert_eq!(after.staged_count(), 1);
    assert!(
        after.generation > opened.snapshot.generation,
        "generation must advance past the pre-mutation snapshot"
    );
    assert!(after.generation >= opened.snapshot.generation.next());
}

#[tokio::test]
async fn stale_generation_is_rejected_for_mutations() {
    let repo = TestRepo::init("stale");
    repo.initial_commit(&[("a.txt", "one\n")]);

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();
    let wrong_generation = Some(Generation(999));

    let err = backend
        .stage_paths(
            opened.id,
            git_backend::api::changes::StageRequest {
                paths: vec!["a.txt".into()],
                all: false,
            },
            wrong_generation,
        )
        .await
        .unwrap_err();

    assert_eq!(err.code(), "staleSnapshot");
}

#[tokio::test]
async fn mutation_bumps_generation_exactly_once() {
    let repo = TestRepo::init("watcher-race");
    repo.initial_commit(&[("a.txt", "one\n"), ("b.txt", "two\n")]);
    repo.write("a.txt", "changed a\n");

    // Default config watches the worktree. The mutation's own `.git/index`
    // write must not trip the watcher: that would bump the generation a
    // second (or, with a burst of events, many) times, racing the
    // frontend's snapshot and spuriously rejecting the next write as stale.
    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();

    backend
        .stage_paths(
            opened.id,
            git_backend::api::changes::StageRequest {
                paths: vec!["a.txt".into()],
                all: false,
            },
            Some(opened.snapshot.generation),
        )
        .await
        .unwrap();

    // Wait out the watcher debounce window, then verify the generation
    // advanced exactly once — from the mutation itself, nothing more.
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    let after = backend
        .repository_snapshot(opened.id)
        .await
        .unwrap()
        .generation;
    assert_eq!(after, opened.snapshot.generation.next());

    // A second write carrying that snapshot must not be rejected as stale:
    // nothing the user did changed the world in between.
    backend
        .stage_paths(
            opened.id,
            git_backend::api::changes::StageRequest {
                paths: vec!["b.txt".into()],
                all: false,
            },
            Some(after),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn gitignored_build_output_does_not_bump_generation() {
    let repo = TestRepo::init("watcher-gitignore");
    repo.initial_commit(&[("README.md", "hi\n")]);
    repo.write(".gitignore", "target\n");
    repo.write("tracked.txt", "one\n");
    repo.commit_all("base files");
    std::fs::create_dir_all(repo.root.join("target/debug")).unwrap();
    std::fs::write(repo.root.join("target/debug/artifact.o"), b"x").unwrap();

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();
    let before = opened.snapshot.generation;

    // Churn inside a gitignored directory (cargo, tsc, bundlers) must not
    // bump the generation: that would race the frontend's snapshot and
    // reject the next write as stale.
    for i in 0..3u8 {
        std::fs::write(repo.root.join("target/debug/artifact.o"), [i]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    let after = backend
        .repository_snapshot(opened.id)
        .await
        .unwrap()
        .generation;
    assert_eq!(
        after, before,
        "gitignored churn must not bump the generation"
    );

    // A real source edit still does.
    std::fs::write(repo.root.join("tracked.txt"), "changed\n").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut current = after;
    while current == after && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        current = backend
            .repository_snapshot(opened.id)
            .await
            .unwrap()
            .generation;
    }
    assert!(current > after, "source edits must bump the generation");
}

#[tokio::test]
async fn globally_ignored_churn_does_not_bump_generation() {
    let repo = TestRepo::init("watcher-global-ignore");
    repo.initial_commit(&[("README.md", "hi\n")]);
    repo.write("tracked.txt", "one\n");
    repo.commit_all("base files");

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();
    let before = opened.snapshot.generation;

    // The override applies per handle, so repos opened before the change
    // observe it without reopening.
    let dir = tempfile::tempdir().unwrap();
    let merged = git_backend::global_ignore::ensure_merged_excludes_file(
        dir.path(),
        "watcher-global-churn-*.tmp\n",
    )
    .unwrap()
    .expect("merged excludes file");
    git_backend::global_ignore::set_global_excludes_file(Some(merged));

    for i in 0..3u8 {
        std::fs::write(repo.root.join("watcher-global-churn-1.tmp"), [i]).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    let after = backend
        .repository_snapshot(opened.id)
        .await
        .unwrap()
        .generation;
    git_backend::global_ignore::set_global_excludes_file(None);
    assert_eq!(
        after, before,
        "globally ignored churn must not bump the generation"
    );

    // A real source edit still does.
    std::fs::write(repo.root.join("tracked.txt"), "changed\n").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut current = after;
    while current == after && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        current = backend
            .repository_snapshot(opened.id)
            .await
            .unwrap()
            .generation;
    }
    assert!(current > after, "source edits must bump the generation");
}

#[tokio::test]
async fn status_classifies_changes_across_areas() {
    let repo = TestRepo::init("status-kinds");
    repo.initial_commit(&[
        ("modified.txt", "original\n"),
        ("deleted.txt", "bye\n"),
        ("renamed-from.txt", "moving on\n"),
    ]);

    repo.write("modified.txt", "changed\n");
    repo.delete("deleted.txt");
    std::fs::rename(
        repo.root.join("renamed-from.txt"),
        repo.root.join("renamed-to.txt"),
    )
    .unwrap();
    repo.write("untracked.txt", "brand new\n");

    repo.stage("untracked.txt");

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();
    let report = backend
        .status(opened.id, StatusOptions::default(), Default::default())
        .await
        .unwrap();

    let by_id = |id: &str| {
        report
            .entries
            .iter()
            .find(|e| e.id == id)
            .unwrap_or_else(|| panic!("missing entry with id {id}"))
    };

    // Every entry id is unique and encodes its side + path.
    let mut ids: Vec<_> = report.entries.iter().map(|e| e.id.clone()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), report.entries.len());

    let modified = by_id("worktree:modified.txt");
    assert_eq!(modified.kind, ChangeKind::Modified);
    assert_eq!(modified.side, ChangeSide::Worktree);

    let deleted = by_id("worktree:deleted.txt");
    assert_eq!(deleted.kind, ChangeKind::Deleted);

    let staged_new = by_id("index:untracked.txt");
    assert_eq!(staged_new.kind, ChangeKind::Added);
    assert_eq!(staged_new.side, ChangeSide::Index);

    assert_eq!(report.staged_count(), 1);
}

#[tokio::test]
async fn history_summaries_carry_stats_tags_and_time() {
    let repo = TestRepo::init("history-summary");
    repo.initial_commit(&[("a.txt", "one\ntwo\n")]);
    let first = repo.head_commit().id();

    // Lightweight tag on the initial commit.
    let target = repo.repo.find_object(first, None).unwrap();
    repo.repo.tag_lightweight("v1", &target, false).unwrap();

    repo.write("a.txt", "one\nchanged\nthree\n");
    repo.write("b.txt", "new file\n");
    repo.commit_all("second");

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();
    let page = backend
        .history_page(opened.id, HistoryPageQuery::default(), Default::default())
        .await
        .unwrap();

    assert!(!page.has_more || page.commits.len() <= 2);
    assert_eq!(page.commits.len(), 2);

    let newest = &page.commits[0];
    assert_eq!(newest.summary_line, "second");
    assert_eq!(newest.files_changed, 2);
    assert!(newest.additions >= 2);
    assert_eq!(newest.deletions, 1);
    assert!(newest.tags.is_empty());
    // Time travels on the signatures (epoch seconds).
    assert!(newest.committer.time_seconds > 0);

    let oldest = &page.commits[1];
    assert_eq!(oldest.summary_line, "initial");
    assert_eq!(oldest.files_changed, 1);
    assert_eq!(oldest.additions, 2);
    assert_eq!(oldest.tags, vec!["v1".to_owned()]);
}

#[tokio::test]
async fn remove_without_trash_closes_session_and_keeps_working_copy() {
    let repo = TestRepo::init("remove-list-only");
    repo.initial_commit(&[("README.md", "# hello")]);

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();
    assert_eq!(backend.open_repository_count(), 1);

    let result = backend.remove_repository(&repo.root, false).await.unwrap();

    assert_eq!(result.removed_repo_id, Some(opened.id.0));
    assert_eq!(
        std::path::Path::new(&result.repo_root),
        repo.root.canonicalize().unwrap()
    );
    assert!(repo.root.exists());
    assert!(repo.root.join(".git").exists());
    assert_eq!(backend.open_repository_count(), 0);
}

#[tokio::test]
async fn remove_without_trash_reports_no_session_for_unopened_repo() {
    let repo = TestRepo::init("remove-unopened");

    let backend = Backend::new(BackendConfig::default());
    let result = backend.remove_repository(&repo.root, false).await.unwrap();

    assert_eq!(result.removed_repo_id, None);
    assert!(repo.root.exists());
}

#[tokio::test]
async fn remove_with_trash_moves_whole_working_copy_from_subfolder_input() {
    let repo = TestRepo::init("remove-trash");
    repo.initial_commit(&[("src/lib.rs", "pub fn f() {}")]);
    let nested = repo.root.join("src");
    // Deconstruct so the git2 handle cannot pin files against the move.
    let TestRepo { root, repo: handle } = repo;
    drop(handle);

    let backend = Backend::new(BackendConfig::default());
    backend.open_repository(&root).await.unwrap();
    let canonical_root = root.canonicalize().unwrap();

    let result = backend.remove_repository(&nested, true).await.unwrap();

    assert_eq!(std::path::Path::new(&result.repo_root), canonical_root);
    assert!(!root.exists(), "repo root should have moved to the trash");
    assert_eq!(backend.open_repository_count(), 0);
}

#[tokio::test]
async fn remove_with_trash_succeeds_when_working_copy_is_already_gone() {
    let dir = tempfile::tempdir().unwrap();
    let ghost = dir.path().join("vanished");
    std::fs::create_dir_all(&ghost).unwrap();
    let ghost_path = ghost.canonicalize().unwrap();
    std::fs::remove_dir_all(&ghost).unwrap();

    let backend = Backend::new(BackendConfig::default());
    let result = backend.remove_repository(&ghost_path, true).await.unwrap();

    assert_eq!(result.removed_repo_id, None);
    assert_eq!(std::path::Path::new(&result.repo_root), ghost_path);
}

/// Mirrors the app at remove time: a git object file is still open (no
/// FILE_SHARE_DELETE) when trash starts; with the session fully released
/// before trash and mutation handles awaited out, trash succeeds directly.
#[cfg(windows)]
#[tokio::test]
async fn remove_with_trash_succeeds_while_a_file_handle_is_open() {
    use std::os::windows::fs::OpenOptionsExt;

    let repo = TestRepo::init("remove-trash-handle");
    repo.initial_commit(&[("probe.txt", "x")]);
    let TestRepo {
        root,
        repo: git_handle,
    } = repo;
    drop(git_handle);

    let probe = root.join("probe.txt");
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&probe)
        .unwrap();

    let backend = Backend::new(BackendConfig::default());
    let _opened = backend.open_repository(&root).await.unwrap();

    let result = backend.remove_repository(&root, true).await;
    drop(file);

    assert!(
        result.is_err(),
        "an incompatible external Windows handle must make trashing fail"
    );
    assert_eq!(result.unwrap_err().code(), "trashFailed");
    assert_eq!(backend.open_repository_count(), 0);
}

#[tokio::test]
async fn status_reads_the_index_fresh_even_when_its_mtime_is_unchanged() {
    let repo = TestRepo::init("status-index-fresh");
    repo.initial_commit(&[("a.txt", "one\n")]);

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();
    let opts = StatusOptions::default();

    let before = backend
        .status(opened.id, opts, Default::default())
        .await
        .unwrap();
    assert_eq!(before.staged_count(), 0);

    // Pin the index mtime back to what the session first observed, then
    // stage. gix's shared index snapshot is keyed on that mtime and would
    // otherwise serve the pre-write index, silently dropping the staged
    // entry from every report; the engine must read the file fresh.
    let index_path = repo.root.join(".git/index");
    let cached_mtime = std::fs::metadata(&index_path).unwrap().modified().unwrap();
    repo.write("b.txt", "new file\n");
    backend
        .stage_paths(
            opened.id,
            git_backend::api::changes::StageRequest {
                paths: vec!["b.txt".into()],
                all: false,
            },
            Some(opened.snapshot.generation),
        )
        .await
        .unwrap();
    std::fs::File::options()
        .write(true)
        .open(&index_path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(cached_mtime))
        .unwrap();

    let after = backend
        .status(opened.id, opts, Default::default())
        .await
        .unwrap();
    assert_eq!(
        after.staged_count(),
        1,
        "staged entry must appear even when the index mtime is unchanged"
    );
}

#[tokio::test]
async fn status_ignores_the_process_cwd_prefix() {
    let repo = TestRepo::init("status-cwd-prefix");
    repo.initial_commit(&[("a.txt", "one\n")]);
    repo.write("b.txt", "new file\n");
    repo.stage("b.txt");

    let subdir = repo.root.join("sub");
    std::fs::create_dir_all(&subdir).unwrap();

    // gix derives the pathspec prefix for tree_index_status from the process
    // CWD at session open; with a CWD inside the worktree it silently dropped
    // every staged entry outside that prefix. Open the repo from a subdir to
    // reproduce the app's CWD, then restore it before status runs.
    let original_cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&subdir).unwrap();
    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();
    std::env::set_current_dir(&original_cwd).unwrap();

    let report = backend
        .status(opened.id, StatusOptions::default(), Default::default())
        .await
        .unwrap();
    assert_eq!(
        report.staged_count(),
        1,
        "staged entries must not depend on the process CWD"
    );
}
