mod common;

use std::path::PathBuf;

use common::TestRepo;
use git_backend::api::changes::StatusOptions;

struct StaticGuard;

impl StaticGuard {
    fn set(path: Option<PathBuf>) -> Self {
        git_backend::global_ignore::set_global_excludes_file(path);
        StaticGuard
    }
}

impl Drop for StaticGuard {
    fn drop(&mut self) {
        git_backend::global_ignore::set_global_excludes_file(None);
    }
}

/// The override is process-global: tests that set it hold this lock for
/// their whole body so parallel tests cannot swap the file mid-assertion.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn untracked_paths(root: &std::path::Path) -> Vec<String> {
    let session = git_backend::engines::gix::GixSession::discover(root).unwrap();
    git_backend::engines::gix::status::status(&session, &StatusOptions::default())
        .unwrap()
        .entries
        .iter()
        .map(|entry| entry.path.as_str().to_owned())
        .collect()
}

#[test]
fn global_excludes_hide_untracked_files() {
    let _serial = serial();
    let repo = TestRepo::init("global-ignore");
    repo.write("global-ignore-probe-one-1.log", "noise\n");
    repo.write("kept.txt", "kept\n");

    assert!(untracked_paths(&repo.root).contains(&"global-ignore-probe-one-1.log".to_owned()));

    let dir = tempfile::tempdir().unwrap();
    let merged = git_backend::global_ignore::ensure_merged_excludes_file(
        dir.path(),
        "global-ignore-probe-one-*.log\n",
    )
    .unwrap()
    .expect("merged excludes file");
    let _guard = StaticGuard::set(Some(merged));

    let paths = untracked_paths(&repo.root);
    assert!(
        !paths.contains(&"global-ignore-probe-one-1.log".to_owned()),
        "globally ignored file still listed: {paths:?}"
    );
    assert!(paths.contains(&"kept.txt".to_owned()));
}

#[test]
fn clearing_the_override_restores_default_behavior() {
    let _serial = serial();
    let repo = TestRepo::init("global-ignore-cleared");
    repo.write("global-ignore-probe-two-2.log", "noise\n");

    let dir = tempfile::tempdir().unwrap();
    let merged = git_backend::global_ignore::ensure_merged_excludes_file(
        dir.path(),
        "global-ignore-probe-two-*.log\n",
    )
    .unwrap()
    .expect("merged excludes file");
    {
        let _guard = StaticGuard::set(Some(merged));
        assert!(!untracked_paths(&repo.root).contains(&"global-ignore-probe-two-2.log".to_owned()));
    }
    assert!(untracked_paths(&repo.root).contains(&"global-ignore-probe-two-2.log".to_owned()));
}

fn git2_should_ignore(root: &std::path::Path, relative: &str) -> bool {
    git2::Repository::open(root)
        .unwrap()
        .status_should_ignore(std::path::Path::new(relative))
        .unwrap()
}

fn gix_is_excluded(root: &std::path::Path, relative: &str) -> bool {
    let session = git_backend::engines::gix::GixSession::discover(root).unwrap();
    session.is_excluded(std::path::Path::new(relative), root.join(relative).is_dir())
}

#[test]
fn watcher_exclusion_matches_git2_reference() {
    let repo = TestRepo::init("exclusion-parity");
    repo.write(".gitignore", "*.log\ntarget/\n!keep.log\n");
    repo.write("sub/.gitignore", "local.tmp\n");
    repo.write("a.log", "noise\n");
    repo.write("keep.log", "kept\n");
    repo.write("tracked.txt", "tracked\n");
    repo.write("sub/local.tmp", "noise\n");
    repo.write("sub/other.txt", "other\n");
    std::fs::create_dir_all(repo.root.join("target/debug")).unwrap();
    repo.write("target/debug/x.o", "object\n");

    for relative in [
        "a.log",
        "keep.log",
        "tracked.txt",
        "target",
        "target/debug/x.o",
        "sub/local.tmp",
        "sub/other.txt",
        "ghost.log",
    ] {
        assert_eq!(
            gix_is_excluded(&repo.root, relative),
            git2_should_ignore(&repo.root, relative),
            "exclusion mismatch for `{relative}`"
        );
    }
    assert!(gix_is_excluded(&repo.root, "a.log"));
    assert!(!gix_is_excluded(&repo.root, "keep.log"));
    assert!(gix_is_excluded(&repo.root, "target"));
}

#[test]
fn global_only_patterns_diverge_from_git2_by_design() {
    let _serial = serial();
    let repo = TestRepo::init("exclusion-global-divergence");
    repo.write("watcher-global-diverge.tmp", "noise\n");

    let dir = tempfile::tempdir().unwrap();
    let merged = git_backend::global_ignore::ensure_merged_excludes_file(
        dir.path(),
        "watcher-global-diverge.tmp\n",
    )
    .unwrap()
    .expect("merged excludes file");
    let _guard = StaticGuard::set(Some(merged));

    assert!(gix_is_excluded(&repo.root, "watcher-global-diverge.tmp"));
    assert!(!git2_should_ignore(
        &repo.root,
        "watcher-global-diverge.tmp"
    ));
}
