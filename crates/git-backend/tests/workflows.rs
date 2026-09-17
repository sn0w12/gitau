mod common;

use common::TestRepo;
use git_backend::api::mutations::{
    MergeContinueRequest, MergeRequest, OperationKind, ResolveConflictRequest, ResolveSide,
    RevertRequest, StashAction, StashPopRequest, StashPushRequest,
};
use git_backend::domain::RevisionSpec;
use git_backend::engines::git2::workflows::{self, WorkflowOutcome};

#[test]
fn fast_forward_merge_moves_branch() {
    let repo = TestRepo::init("merge-ff");
    repo.initial_commit(&[("base.txt", "base\n")]);
    let base = repo.head_commit().id();

    let branch = repo
        .repo
        .branch("feature", &repo.head_commit(), false)
        .unwrap();
    repo.repo.set_head(branch.get().name().unwrap()).unwrap();
    repo.write("feature.txt", "from feature\n");
    repo.commit_all("feature work");
    repo.repo.set_head_detached(base).unwrap();
    repo.repo
        .find_reference(&git2_branch_ref("master"))
        .or_else(|_| repo.repo.find_reference(&git2_branch_ref("main")))
        .unwrap()
        .set_target(base, "rewind master")
        .unwrap();

    let session_repo = git2::Repository::discover(&repo.root).unwrap();
    let outcome = workflows::merge(
        &session_repo,
        &MergeRequest {
            target: RevisionSpec::parse("refs/heads/feature").unwrap(),
            fast_forward_only: true,
            no_fast_forward: false,
            message: None,
        },
    )
    .unwrap();

    assert!(matches!(outcome, WorkflowOutcome::FastForwarded { .. }));
}

fn git2_branch_ref(name: &str) -> String {
    format!("refs/heads/{name}")
}

#[test]
fn merge_conflict_flow() {
    let repo = TestRepo::init("conflict-flow");
    repo.initial_commit(&[("conflict.txt", "base\n")]);
    let base_oid = repo.head_commit().id();
    let base_commit = repo.repo.find_commit(base_oid).unwrap();

    let branch = repo.repo.branch("side", &base_commit, false).unwrap();
    repo.repo.set_head(branch.get().name().unwrap()).unwrap();
    repo.write("conflict.txt", "side version\n");
    repo.commit_all("side change");

    let default_branch = default_branch_ref(&repo);
    repo.repo
        .find_reference(default_branch)
        .unwrap()
        .set_target(base_oid, "back to base")
        .unwrap();
    repo.repo.set_head(default_branch).unwrap();
    force_checkout_head(&repo);

    repo.write("conflict.txt", "main version\n");
    repo.stage("conflict.txt");
    repo.commit_all("main change");

    let session_repo = git2::Repository::discover(&repo.root).unwrap();
    let outcome = workflows::merge(
        &session_repo,
        &MergeRequest {
            target: RevisionSpec::parse("refs/heads/side").unwrap(),
            ..Default::default()
        },
    )
    .unwrap();

    match outcome {
        WorkflowOutcome::Conflicted { paths } => {
            assert!(paths.iter().any(|p| p == "conflict.txt"));
        }
        other => panic!("expected conflicts, got {other:?}"),
    }

    assert!(!workflows_status_clean(&repo));

    repo.write("conflict.txt", "resolved\n");
    repo.stage("conflict.txt");
    {
        let probe = git2::Repository::discover(&repo.root).unwrap();
        let idx = probe.index().unwrap();
        println!(
            "DEBUG has_conflicts={} staged_content={:?}",
            idx.has_conflicts(),
            std::fs::read_to_string(repo.root.join("conflict.txt")).ok()
        );
    }

    let finished =
        workflows::merge_continue(&session_repo, &MergeContinueRequest { message: None }).unwrap();
    match finished {
        WorkflowOutcome::Finished { commit } => assert!(commit.is_some()),
        other => panic!("expected finish, got {other:?}"),
    }
    let head_msg = repo.head_commit().message().unwrap().to_owned();
    assert!(head_msg.contains("Merge"));
}

fn force_checkout_head(repo: &TestRepo) {
    let mut builder = git2::build::CheckoutBuilder::new();
    builder.force();
    let head_commit = repo.head_commit();
    repo.repo
        .checkout_tree(head_commit.as_object(), Some(&mut builder))
        .unwrap();
}

fn workflows_status_clean(repo: &TestRepo) -> bool {
    let opts = git_backend::api::changes::StatusOptions::default();
    let session = git_backend::engines::git2::Git2Session::new(&repo.root);
    let report = git_backend::engines::git2::status::status(&session, &opts).unwrap();
    report.entries.is_empty() && report.conflicts.is_empty()
}

#[test]
fn merge_abort_restores_clean_state() {
    let repo = TestRepo::init("abort-flow");
    repo.initial_commit(&[("conflict.txt", "base\n")]);
    let base_oid = repo.head_commit().id();
    let base_commit = repo.repo.find_commit(base_oid).unwrap();

    let branch = repo.repo.branch("side", &base_commit, false).unwrap();
    repo.repo.set_head(branch.get().name().unwrap()).unwrap();
    repo.write("conflict.txt", "side\n");
    repo.commit_all("side change");

    let default_branch = default_branch_ref(&repo);
    repo.repo
        .find_reference(default_branch)
        .unwrap()
        .set_target(base_oid, "back")
        .unwrap();
    repo.repo.set_head(default_branch).unwrap();
    force_checkout_head(&repo);

    repo.write("conflict.txt", "main\n");
    repo.stage("conflict.txt");
    repo.commit_all("main change");

    let session_repo = git2::Repository::discover(&repo.root).unwrap();
    let outcome = workflows::merge(
        &session_repo,
        &MergeRequest {
            target: RevisionSpec::parse("refs/heads/side").unwrap(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(outcome, WorkflowOutcome::Conflicted { .. }));

    let aborted = workflows::merge_abort(&session_repo).unwrap();
    assert_eq!(aborted, WorkflowOutcome::Aborted);
    assert!(!session_repo.path().join("MERGE_HEAD").exists());
    assert!(workflows_status_clean(&repo));
}

#[test]
fn stash_round_trip_preserves_worktree_changes() {
    let repo = TestRepo::init("stash-flow");
    repo.initial_commit(&[("a.txt", "committed\n")]);
    repo.write("a.txt", "uncommitted work\n");
    repo.write("new.txt", "untracked\n");

    let mut session_repo = git2::Repository::discover(&repo.root).unwrap();
    workflows::stash_push(
        &mut session_repo,
        &StashPushRequest {
            message: Some("wip".into()),
            include_untracked: true,
            keep_index: false,
            paths: Vec::new(),
        },
    )
    .unwrap();

    assert_eq!(repo.read("a.txt"), "committed\n");
    assert!(!repo.root.join("new.txt").exists());

    let entries = workflows::stash_list(&mut session_repo).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].message.contains("wip"));

    workflows::stash_pop(
        &mut session_repo,
        &StashPopRequest {
            index: 0,
            action: StashAction::Pop,
        },
    )
    .unwrap();

    assert_eq!(repo.read("a.txt"), "uncommitted work\n");
    assert!(repo.root.join("new.txt").exists());
}

#[test]
fn path_stash_reverts_only_the_selected_path() {
    let repo = TestRepo::init("stash-paths");
    repo.initial_commit(&[("a.txt", "a0\n"), ("b.txt", "b0\n")]);
    repo.write("a.txt", "a1\n");
    repo.write("b.txt", "b1\n");

    let mut session_repo = git2::Repository::discover(&repo.root).unwrap();
    workflows::stash_push(
        &mut session_repo,
        &StashPushRequest {
            message: Some("just a".into()),
            include_untracked: false,
            keep_index: false,
            paths: vec!["a.txt".into()],
        },
    )
    .unwrap();

    assert_eq!(repo.read("a.txt"), "a0\n");
    assert_eq!(repo.read("b.txt"), "b1\n");

    let entries = workflows::stash_list(&mut session_repo).unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0].message.contains("just a"));

    workflows::stash_pop(
        &mut session_repo,
        &StashPopRequest {
            index: 0,
            action: StashAction::Pop,
        },
    )
    .unwrap();

    assert_eq!(repo.read("a.txt"), "a1\n");
    assert_eq!(repo.read("b.txt"), "b1\n");
}

#[test]
fn path_stash_round_trips_a_deletion() {
    let repo = TestRepo::init("stash-paths-delete");
    repo.initial_commit(&[("a.txt", "a0\n"), ("b.txt", "b0\n")]);
    repo.delete("a.txt");
    repo.write("b.txt", "b1\n");

    let mut session_repo = git2::Repository::discover(&repo.root).unwrap();
    workflows::stash_push(
        &mut session_repo,
        &StashPushRequest {
            message: None,
            include_untracked: false,
            keep_index: false,
            paths: vec!["a.txt".into()],
        },
    )
    .unwrap();

    assert!(repo.root.join("a.txt").exists());
    assert_eq!(repo.read("a.txt"), "a0\n");
    assert_eq!(repo.read("b.txt"), "b1\n");

    workflows::stash_pop(
        &mut session_repo,
        &StashPopRequest {
            index: 0,
            action: StashAction::Pop,
        },
    )
    .unwrap();

    assert!(!repo.root.join("a.txt").exists());
    assert_eq!(repo.read("b.txt"), "b1\n");
}

#[test]
fn path_stash_requires_include_untracked_for_untracked_files() {
    let repo = TestRepo::init("stash-paths-untracked");
    repo.initial_commit(&[("a.txt", "a0\n")]);
    repo.write("new.txt", "new\n");

    let mut session_repo = git2::Repository::discover(&repo.root).unwrap();
    let result = workflows::stash_push(
        &mut session_repo,
        &StashPushRequest {
            message: None,
            include_untracked: false,
            keep_index: false,
            paths: vec!["new.txt".into()],
        },
    );
    assert!(result.is_err());
    assert!(repo.root.join("new.txt").exists());
}

#[test]
fn path_stash_round_trips_an_untracked_file() {
    let repo = TestRepo::init("stash-paths-untracked-ok");
    repo.initial_commit(&[("a.txt", "a0\n")]);
    repo.write("a.txt", "a1\n");
    repo.write("new.txt", "new\n");

    let mut session_repo = git2::Repository::discover(&repo.root).unwrap();
    workflows::stash_push(
        &mut session_repo,
        &StashPushRequest {
            message: None,
            include_untracked: true,
            keep_index: false,
            paths: vec!["new.txt".into()],
        },
    )
    .unwrap();

    assert_eq!(repo.read("a.txt"), "a1\n");
    assert!(!repo.root.join("new.txt").exists());

    // The first-parent tree must carry the untracked file, otherwise the
    // commit-to-parent diff used by the stash viewer is empty.
    let carries_untracked = {
        let entries = workflows::stash_list(&mut session_repo).unwrap();
        let stash_commit = session_repo
            .find_commit(git2::Oid::from_str(&entries[0].commit).unwrap())
            .unwrap();
        stash_commit
            .tree()
            .unwrap()
            .get_path(std::path::Path::new("new.txt"))
            .is_ok()
    };
    assert!(carries_untracked);

    workflows::stash_pop(
        &mut session_repo,
        &StashPopRequest {
            index: 0,
            action: StashAction::Pop,
        },
    )
    .unwrap();

    assert_eq!(repo.read("a.txt"), "a1\n");
    assert!(repo.root.join("new.txt").exists());
}

#[test]
fn cherry_pick_and_revert_create_expected_commits() {
    let repo = TestRepo::init("pick-revert");
    repo.initial_commit(&[("base.txt", "b\n")]);
    let base = repo.head_commit().id();

    let branch = repo
        .repo
        .branch("picker", &repo.head_commit(), false)
        .unwrap();
    repo.repo.set_head(branch.get().name().unwrap()).unwrap();
    repo.write("picked.txt", "picked content\n");
    let picked_oid = repo.commit_all("the pick");

    repo.repo
        .find_reference(default_branch_ref(&repo))
        .unwrap()
        .set_target(base, "back")
        .unwrap();
    repo.repo.set_head(default_branch_ref(&repo)).unwrap();
    force_checkout_head(&repo);
    drop(branch);

    let session_repo = git2::Repository::discover(&repo.root).unwrap();
    let picked = workflows::cherry_pick(
        &session_repo,
        &RevisionSpec::from_oid(
            git_backend::domain::ObjectId::from_bytes(picked_oid.as_bytes()).unwrap(),
        ),
    )
    .unwrap();
    let WorkflowOutcome::Finished { commit } = picked else {
        panic!("cherry-pick should finish cleanly");
    };
    assert!(commit.is_some());
    assert!(repo.root.join("picked.txt").exists());

    let head_before_revert = repo.head_commit().id();
    let reverted = workflows::revert(
        &session_repo,
        &RevertRequest {
            target: RevisionSpec::from_oid(
                git_backend::domain::ObjectId::from_bytes(head_before_revert.as_bytes()).unwrap(),
            ),
            parent_index: 0,
        },
    )
    .unwrap();
    assert!(matches!(reverted, WorkflowOutcome::Finished { .. }));
    assert!(!repo.root.join("picked.txt").exists());
}

#[test]
fn merge_operation_state_reports_conflicts() {
    let repo = TestRepo::init("merge-operation-state");
    repo.initial_commit(&[("conflict.txt", "base\n")]);
    let (side, default_branch) = make_conflicting_merge(&repo);

    let session_repo = git2::Repository::discover(&repo.root).unwrap();
    let outcome = workflows::merge(
        &session_repo,
        &MergeRequest {
            target: RevisionSpec::parse(&format!("refs/heads/{side}")).unwrap(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(outcome, WorkflowOutcome::Conflicted { .. }));
    drop(side);
    drop(default_branch);

    let state = workflows::operation_state(&session_repo).unwrap();
    assert_eq!(state.kind, OperationKind::Merge);
    assert_eq!(state.conflict_paths, vec!["conflict.txt".to_owned()]);
    assert_eq!(state.heads.len(), 1);

    let base = workflows::conflict_file(&session_repo, "conflict.txt", 1).unwrap();
    let ours = workflows::conflict_file(&session_repo, "conflict.txt", 2).unwrap();
    let theirs = workflows::conflict_file(&session_repo, "conflict.txt", 3).unwrap();
    assert_eq!(base.stage, 1);
    assert!(!base.binary);
    assert!(!base.truncated);
    let text_of = |rows: &[git_backend::streaming::model::DiffRow]| {
        rows.iter()
            .map(|row| row.content.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(text_of(&base.rows).contains("base"));
    assert!(text_of(&ours.rows).contains("main version"));
    assert!(text_of(&theirs.rows).contains("side version"));
}

#[test]
fn resolve_conflict_theirs_clears_index_entry() {
    let repo = TestRepo::init("merge-resolve-theirs");
    repo.initial_commit(&[("conflict.txt", "base\n")]);
    let (side, default_branch) = make_conflicting_merge(&repo);

    let session_repo = git2::Repository::discover(&repo.root).unwrap();
    workflows::merge(
        &session_repo,
        &MergeRequest {
            target: RevisionSpec::parse(&format!("refs/heads/{side}")).unwrap(),
            ..Default::default()
        },
    )
    .unwrap();
    drop(side);
    drop(default_branch);

    workflows::resolve_conflict(
        &session_repo,
        &ResolveConflictRequest {
            path: "conflict.txt".into(),
            side: ResolveSide::Theirs,
        },
    )
    .unwrap();

    assert_eq!(repo.read("conflict.txt"), "side version\n");
    let index = session_repo.index().unwrap();
    assert!(!index.has_conflicts());

    let finished =
        workflows::merge_continue(&session_repo, &MergeContinueRequest { message: None }).unwrap();
    assert!(matches!(finished, WorkflowOutcome::Finished { .. }));
    let state = workflows::operation_state(&session_repo).unwrap();
    assert_eq!(state.kind, OperationKind::None);
    assert!(state.conflict_paths.is_empty());
}

#[test]
fn operation_state_is_empty_without_merge() {
    let repo = TestRepo::init("merge-operation-empty");
    repo.initial_commit(&[("a.txt", "a\n")]);
    let session_repo = git2::Repository::discover(&repo.root).unwrap();
    let state = workflows::operation_state(&session_repo).unwrap();
    assert_eq!(state.kind, OperationKind::None);
    assert!(state.conflict_paths.is_empty());
}

fn make_conflicting_merge(repo: &TestRepo) -> (String, String) {
    let base_oid = repo.head_commit().id();
    let base_commit = repo.repo.find_commit(base_oid).unwrap();
    let branch = repo.repo.branch("side", &base_commit, false).unwrap();
    let side_name = branch
        .get()
        .name()
        .unwrap()
        .strip_prefix("refs/heads/")
        .unwrap()
        .to_owned();
    repo.repo.set_head(branch.get().name().unwrap()).unwrap();
    repo.write("conflict.txt", "side version\n");
    repo.commit_all("side change");

    let default_branch = default_branch_ref(repo).to_owned();
    repo.repo
        .find_reference(&default_branch)
        .unwrap()
        .set_target(base_oid, "back to base")
        .unwrap();
    repo.repo.set_head(&default_branch).unwrap();
    force_checkout_head(repo);

    repo.write("conflict.txt", "main version\n");
    repo.stage("conflict.txt");
    repo.commit_all("main change");
    (side_name, default_branch)
}

fn default_branch_ref(repo: &TestRepo) -> &'static str {
    if repo.repo.find_reference("refs/heads/master").is_ok() {
        "refs/heads/master"
    } else {
        "refs/heads/main"
    }
}
