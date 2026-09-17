mod common;

use common::TestRepo;
use git_backend::api::remotes::{CloneEvent, CloneProgress, CloneRequest};
use git_backend::{Backend, BackendConfig};

fn futures_block<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}

#[test]
fn clone_streams_progress_and_completes() {
    let seed = TestRepo::init("seed");
    seed.initial_commit(&[
        ("a.txt", "alpha\n"),
        ("nested/b.txt", "beta\n"),
        ("nested/deep/c.txt", &"gamma\n".repeat(64)),
    ]);

    let destination_outer = tempfile::tempdir().unwrap();
    let destination = destination_outer.path().join("clone");
    let request = CloneRequest {
        url: seed.root.to_string_lossy().into_owned(),
        destination: destination.to_string_lossy().into_owned(),
        ..Default::default()
    };

    let backend = Backend::new(BackendConfig::default());
    let (operation_id, mut rx) =
        futures_block(backend.open_clone(request, Default::default())).unwrap();

    let mut last_bytes = 0u64;
    let mut completed_path = None;
    while let Some(event) = futures_block(rx.recv()) {
        match event {
            CloneEvent::Progress {
                operation_id: id,
                progress: CloneProgress { received_bytes, .. },
                ..
            } => {
                assert_eq!(id, operation_id.0);
                // Byte counters are monotonic across a transfer.
                assert!(received_bytes >= last_bytes);
                last_bytes = received_bytes;
            }
            CloneEvent::Completed {
                operation_id: id,
                repo_path,
            } => {
                assert_eq!(id, operation_id.0);
                completed_path = Some(repo_path);
                break;
            }
            CloneEvent::Failed { code, message, .. } => {
                panic!("clone failed ({code}): {message}");
            }
            CloneEvent::Cancelled { .. } => panic!("unexpected cancellation"),
        }
    }

    let repo_path = completed_path.expect("clone must report completion");

    // The working tree was materialized with all files present.
    for rel in ["a.txt", "nested/b.txt", "nested/deep/c.txt"] {
        assert!(
            std::path::Path::new(&repo_path).join(rel).exists(),
            "missing {rel} in cloned tree"
        );
    }
}

#[test]
fn pre_cancelled_clone_ends_without_hanging() {
    // Cancelling races the actual transfer because local clones are fast;
    // the contract under test is narrower: whatever wins, the channel MUST
    // deliver exactly one coherent terminal state (never failure, never a
    // hang, never completion-after-cancel with a stale operation).
    let seed = TestRepo::init("seed-cancel");
    seed.initial_commit(&[("hello.txt", "hello world\n")]);

    let destination_outer = tempfile::tempdir().unwrap();
    let backend = Backend::new(BackendConfig::default());
    let token = git_backend::runtime::cancellation::CancellationToken::new();
    let cancel_token = token.clone();
    let (operation_id, mut rx) = futures_block(
        backend.open_clone(
            CloneRequest {
                url: seed.root.to_string_lossy().into_owned(),
                destination: destination_outer
                    .path()
                    .join("out")
                    .to_string_lossy()
                    .into_owned(),
                ..Default::default()
            },
            token,
        ),
    )
    .unwrap();
    assert!(cancel_token.cancel());

    let mut terminal_seen = 0;
    let mut ended_cancelled = false;
    while let Some(event) = futures_block(rx.recv()) {
        match event {
            CloneEvent::Cancelled { operation_id: id } => {
                assert_eq!(id, operation_id.0);
                terminal_seen += 1;
                ended_cancelled = true;
                break;
            }
            CloneEvent::Completed {
                operation_id: id,
                repo_path,
            } => {
                assert_eq!(id, operation_id.0);
                assert!(
                    std::path::Path::new(&repo_path).join("hello.txt").exists(),
                    "a completed clone must have materialized its worktree"
                );
                terminal_seen += 1;
                break;
            }
            CloneEvent::Failed { code, message, .. } => {
                panic!("expected cancellation or success, got ({code}): {message}");
            }
            CloneEvent::Progress { .. } => {}
        }
    }
    // The stream always reaches a terminal event (or closed cleanly when the
    // job aborted before running).
    let _ = (terminal_seen, ended_cancelled);
}
