use git_backend::api::worktrees::WorktreeCreateRequest;
use git_backend::engines::git2::worktrees::{
    create_worktree, list_worktrees, lock_worktree, remove_worktree, unlock_worktree,
};

mod common;

use common::TestRepo;

#[test]
fn lists_only_primary_tree_for_plain_repository() {
    let repo = TestRepo::init("plain");
    repo.initial_commit(&[("README.md", "hello\n")]);

    let trees = list_worktrees(&repo.repo).unwrap();
    assert_eq!(trees.len(), 1);
    assert_eq!(trees[0].name, "main");
    assert!(trees[0].is_current);
    assert_eq!(trees[0].branch.as_deref(), Some("master"));
}

#[test]
fn worktree_round_trip_create_list_lock_remove() {
    let repo = TestRepo::init("linked");
    repo.initial_commit(&[("README.md", "hello\n")]);

    let head = repo.head_commit().id();
    let head_obj = repo.repo.find_commit(head).unwrap();
    let branch = repo.repo.branch("feature", &head_obj, false).unwrap();
    assert!(branch.into_reference().is_branch());

    // Create a linked tree attached to the existing branch.
    let created = create_worktree(
        &repo.repo,
        &WorktreeCreateRequest {
            name: "wt-feature".to_owned(),
            path: None,
            start_point: Some("feature".to_owned()),
        },
    )
    .unwrap();

    assert_eq!(created.name, "wt-feature");
    assert!(!created.is_current);
    assert_eq!(created.branch.as_deref(), Some("feature"));
    let wt_path = std::path::PathBuf::from(&created.path);

    // Materialized: opening the tree lets us read the committed file.
    let wt_repo = git2::Repository::open(&created.path).expect("linked tree directory exists");
    assert_eq!(
        std::fs::read_to_string(wt_path.join("README.md")).unwrap(),
        "hello\n"
    );

    let trees = list_worktrees(&repo.repo).unwrap();
    assert_eq!(trees.len(), 2);
    let linked = trees.iter().find(|t| t.name == "wt-feature").unwrap();
    assert_eq!(linked.branch.as_deref(), Some("feature"));
    assert!(!linked.is_prunable);

    lock_worktree(&repo.repo, "wt-feature", "in use").unwrap();
    let wt = repo.repo.find_worktree("wt-feature").unwrap();
    assert_eq!(
        wt.is_locked().unwrap(),
        git2::WorktreeLockStatus::Locked(Some("in use".to_owned()))
    );
    unlock_worktree(&repo.repo, "wt-feature").unwrap();
    assert_eq!(wt.is_locked().unwrap(), git2::WorktreeLockStatus::Unlocked);

    drop(wt_repo);
    remove_worktree(&repo.repo, "wt-feature", true).unwrap();
    assert!(!wt_path.exists());

    let trees = list_worktrees(&repo.repo).unwrap();
    assert_eq!(trees.len(), 1);
    assert_eq!(trees[0].name, "main");
}

#[test]
fn prunes_stale_worktree_whose_directory_was_deleted() {
    let repo = TestRepo::init("stale");
    repo.initial_commit(&[("README.md", "hello\n")]);

    let created = create_worktree(
        &repo.repo,
        &WorktreeCreateRequest {
            name: "wt-gone".to_owned(),
            path: None,
            start_point: None,
        },
    )
    .unwrap();

    // Simulate an external delete of the checkout directory.
    std::fs::remove_dir_all(&created.path).unwrap();

    let linked = list_worktrees(&repo.repo)
        .unwrap()
        .into_iter()
        .find(|t| t.name == "wt-gone")
        .expect("linked tree still listed");
    assert!(linked.is_prunable);

    remove_worktree(&repo.repo, "wt-gone", false).unwrap();
    let names: Vec<String> = list_worktrees(&repo.repo)
        .unwrap()
        .into_iter()
        .map(|t| t.name)
        .collect();
    assert_eq!(names, vec!["main"]);
}
