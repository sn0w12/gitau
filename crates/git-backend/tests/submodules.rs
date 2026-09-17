use git_backend::api::submodules::{SubmoduleAddRequest, SubmoduleUpdateRequest};
use git_backend::engines::git2::submodules::{add_submodule, list_submodules, update_submodule};

mod common;

use common::TestRepo;

#[test]
fn add_and_list_submodule_materializes_content() {
    // A standalone repo acts as the submodule's remote; its path is used as
    // the clone URL so no network is involved.
    let child = TestRepo::init("child");
    child.initial_commit(&[("file.txt", "hello from child\n")]);

    let super_repo = TestRepo::init("super");
    super_repo.initial_commit(&[("README.md", "super\n")]);

    let url = child.root.to_string_lossy().replace('\\', "/");

    add_submodule(
        &super_repo.repo,
        &SubmoduleAddRequest {
            url,
            path: "vendor/child".to_owned(),
            branch: None,
        },
    )
    .unwrap();

    let subs = list_submodules(&super_repo.repo).unwrap();
    assert_eq!(subs.len(), 1);
    let sub = &subs[0];
    assert_eq!(sub.path, "vendor/child");
    assert!(sub.initialized);
    assert!(sub.checked_out);
    assert!(!sub.dirty);
    assert!(sub.commit.is_some());

    // The nested content is materialized in the super worktree. Normalize
    // line endings: the submodule clone may apply the ambient autocrlf rule.
    let content = std::fs::read_to_string(super_repo.root.join("vendor/child/file.txt")).unwrap();
    assert_eq!(content.replace("\r\n", "\n"), "hello from child\n");
}

#[test]
fn update_named_submodule_is_a_noop_when_clean() {
    let child = TestRepo::init("child2");
    child.initial_commit(&[("a.txt", "a\n")]);

    let super_repo = TestRepo::init("super2");
    super_repo.initial_commit(&[("README.md", "super\n")]);

    add_submodule(
        &super_repo.repo,
        &SubmoduleAddRequest {
            url: child.root.to_string_lossy().replace('\\', "/"),
            path: "sub".to_owned(),
            branch: None,
        },
    )
    .unwrap();

    // Re-running update keeps the submodule initialized and clean.
    update_submodule(
        &super_repo.repo,
        &SubmoduleUpdateRequest {
            names: vec!["sub".to_owned()],
            recursive: false,
            init: true,
        },
    )
    .unwrap();

    let subs = list_submodules(&super_repo.repo).unwrap();
    assert_eq!(subs.len(), 1);
    assert!(subs[0].initialized);
    assert!(!subs[0].dirty);
}
