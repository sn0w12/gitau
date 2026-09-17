//! Differential tests proving the gix status engine produces the same
//! StatusReport as the libgit2 reference engine across scenarios.

mod common;

use common::TestRepo;
use git_backend::api::changes::StatusOptions;
use git_backend::engines::git2::session::Git2Session;
use git_backend::engines::gix::GixSession;

fn assert_parity(repo: &TestRepo, opts: StatusOptions) {
    let git2_report =
        git_backend::engines::git2::status::status(&Git2Session::new(repo.root.clone()), &opts)
            .unwrap();
    let gix_report = git_backend::engines::gix::status::status(
        &GixSession::discover(&repo.root).unwrap(),
        &opts,
    )
    .unwrap();

    assert_eq!(
        git2_report.entries, gix_report.entries,
        "entries mismatch (git2 = reference)"
    );
    assert_eq!(
        git2_report.conflicts, gix_report.conflicts,
        "conflicts mismatch (git2 = reference)"
    );
}

#[test]
fn clean_repo_has_no_entries() {
    let repo = TestRepo::init("parity-clean");
    repo.initial_commit(&[("a.txt", "one\n"), ("b.txt", "two\n")]);
    assert_parity(&repo, StatusOptions::default());
}

#[test]
fn modified_and_deleted_files_match() {
    let repo = TestRepo::init("parity-moddel");
    repo.initial_commit(&[("a.txt", "one\n"), ("b.txt", "two\n"), ("c.txt", "three\n")]);
    repo.write("a.txt", "changed\n");
    repo.delete("b.txt");
    assert_parity(&repo, StatusOptions::default());
}

#[test]
fn staged_modification_matches() {
    let repo = TestRepo::init("parity-staged");
    repo.initial_commit(&[("a.txt", "one\n")]);
    repo.write("a.txt", "staged change\n");
    repo.stage("a.txt");
    assert_parity(&repo, StatusOptions::default());
}

#[test]
fn staged_add_on_unborn_head_matches() {
    let repo = TestRepo::init("parity-unborn-staged");
    repo.write("a.txt", "staged before first commit\n");
    repo.stage("a.txt");
    assert_parity(&repo, StatusOptions::default());
}

#[test]
fn staged_add_and_untracked_match() {
    let repo = TestRepo::init("parity-adduntracked");
    repo.initial_commit(&[("a.txt", "one\n")]);
    repo.write("new.txt", "untracked\n");
    repo.write("staged-new.txt", "staged\n");
    repo.stage("staged-new.txt");
    repo.write("nested/deep/file.txt", "deep\n");
    assert_parity(&repo, StatusOptions::default());
}

#[test]
fn untracked_without_recursion_collapses_dirs() {
    let repo = TestRepo::init("parity-norecurse");
    repo.initial_commit(&[("a.txt", "one\n")]);
    repo.write("nested/deep/file.txt", "deep\n");
    repo.write("loose.txt", "loose\n");

    let opts = StatusOptions {
        recurse_untracked_dirs: false,
        ..StatusOptions::default()
    };
    // Sanity-check both engines actually report the collapsed directory.
    for engine in ["git2", "gix"] {
        let report = if engine == "git2" {
            git_backend::engines::git2::status::status(&Git2Session::new(repo.root.clone()), &opts)
                .unwrap()
        } else {
            git_backend::engines::gix::status::status(
                &GixSession::discover(&repo.root).unwrap(),
                &opts,
            )
            .unwrap()
        };
        assert!(
            !report.entries.is_empty(),
            "{engine} must surface untracked content"
        );
    }
    assert_parity(&repo, opts);
}

#[test]
fn worktree_rename_matches() {
    let repo = TestRepo::init("parity-wt-rename");
    repo.initial_commit(&[(
        "original-name.txt",
        "content to keep similar\n".repeat(8).as_str(),
    )]);
    std::fs::rename(
        repo.root.join("original-name.txt"),
        repo.root.join("renamed-file.txt"),
    )
    .unwrap();
    assert_parity(&repo, StatusOptions::default());
}

#[test]
fn staged_rename_matches() {
    let repo = TestRepo::init("parity-staged-rename");
    repo.initial_commit(&[("before.txt", "stable contents\n".repeat(6).as_str())]);
    std::fs::rename(repo.root.join("before.txt"), repo.root.join("after.txt")).unwrap();
    {
        let mut index = repo.repo.index().unwrap();
        index
            .remove_path(std::path::Path::new("before.txt"))
            .unwrap();
        index
            .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
    }
    assert_parity(&repo, StatusOptions::default());
}

#[test]
fn file_replaced_by_directory_typechange_matches() {
    let repo = TestRepo::init("parity-typechange");
    repo.initial_commit(&[("thing", "i am a file\n")]);
    repo.delete("thing");
    std::fs::create_dir_all(repo.root.join("thing")).unwrap();
    repo.write("thing/inner.txt", "now a dir\n");
    assert_parity(&repo, StatusOptions::default());
}

#[test]
fn merge_conflict_classification_matches() {
    let repo = TestRepo::init("parity-conflict");
    repo.initial_commit(&[("shared.txt", "base\n")]);

    let base = repo.head_commit().id();
    let base_commit = repo.repo.find_commit(base).unwrap();
    repo.repo.branch("feature", &base_commit, false).unwrap();

    // Main side changes shared.txt one way.
    repo.write("shared.txt", "main version\n");
    repo.commit_all("main change");
    let main_tip = repo.head_commit().id();

    // Feature side (branched from base) changes shared.txt another way.
    checkout_ref(&repo, "refs/heads/feature");
    repo.write("shared.txt", "feature version\n");
    repo.commit_all("feature change");

    // Merge main into feature; shared.txt conflicts.
    let feature_head = repo.head_commit().id();
    let main_commit = repo.repo.find_commit(main_tip).unwrap();
    let feature_commit = repo.repo.find_commit(feature_head).unwrap();
    let mut index = repo
        .repo
        .merge_commits(&feature_commit, &main_commit, None)
        .unwrap();
    assert!(index.has_conflicts());
    repo.repo.checkout_index(Some(&mut index), None).unwrap();

    assert_parity(&repo, StatusOptions::default());
}

fn checkout_ref(repo: &TestRepo, refname: &str) {
    let reference = repo.repo.find_reference(refname).unwrap();
    let object = reference.peel(git2::ObjectType::Commit).unwrap();
    repo.repo
        .reset(&object, git2::ResetType::Hard, None)
        .unwrap();
}

#[test]
fn ignored_files_stay_absent_with_default_options() {
    let repo = TestRepo::init("parity-ignored");
    repo.initial_commit(&[("a.txt", "one\n")]);
    repo.write(".gitignore", "ignored*\n");
    repo.write("ignored.log", "noise\n");
    assert_parity(&repo, StatusOptions::default());
}
