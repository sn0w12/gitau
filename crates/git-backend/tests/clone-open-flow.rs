mod common;

use common::TestRepo;
use git_backend::api::remotes::{CloneEvent, CloneRequest};
use git_backend::{Backend, BackendConfig};

fn futures_block<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}

/// The exact app sequence after the user presses Clone: run the clone
/// through the operation channel, take the completed repo_path, and open
/// that path the way `git_open_repository` does.
#[test]
fn completed_clone_path_opens_immediately() {
    let seed = TestRepo::init("seed");
    seed.initial_commit(&[("hello.txt", "hello world\n")]);

    let destination_outer = tempfile::tempdir().unwrap();
    let destination = destination_outer.path().join("clone");
    let backend = Backend::new(BackendConfig::default());

    let (operation_id, mut rx) = futures_block(backend.open_clone(
        CloneRequest {
            url: seed.root.to_string_lossy().into_owned(),
            destination: destination.to_string_lossy().into_owned(),
            ..Default::default()
        },
        Default::default(),
    ))
    .unwrap();

    let mut completed_path = None;
    while let Some(event) = futures_block(rx.recv()) {
        match event {
            CloneEvent::Completed { repo_path, .. } => {
                assert!(!repo_path.is_empty());
                completed_path = Some(repo_path);
                break;
            }
            CloneEvent::Progress { .. } => {}
            CloneEvent::Failed { code, message, .. } => {
                panic!("clone failed ({code}): {message}");
            }
            CloneEvent::Cancelled { .. } => panic!("unexpected cancel"),
        }
    }
    let _ = operation_id;

    let repo_path = completed_path.expect("clone must complete");
    let opened = futures_block(backend.open_repository(std::path::Path::new(&repo_path)))
        .expect("opening the cloned path must succeed");
    assert!(opened.id.0 > 0);

    // Reopening is idempotent (frontend openRepositoryByPath is a no-op on a
    // known path, but the backend must also dedupe).
    let again = futures_block(backend.open_repository(std::path::Path::new(&repo_path))).unwrap();
    assert_eq!(again.id, opened.id);
}
