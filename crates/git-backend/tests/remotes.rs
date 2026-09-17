mod common;

use std::path::PathBuf;

use common::TestRepo;
use git_backend::api::changes::StatusOptions;
use git_backend::api::remotes::{
    CloneEvent, CloneRequest, PullRequest, PushRequest, RemoteAddRequest,
};
use git_backend::{Backend, BackendConfig};

fn futures_block<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Runtime::new().unwrap().block_on(future)
}

/// Starts a clone through the backend and pumps the event channel to its
/// terminal state, mirroring what the Tauri command does for IPC.
fn run_clone_to_completion(backend: &Backend, request: CloneRequest) -> PathBuf {
    let (operation_id, mut rx) =
        futures_block(backend.open_clone(request, Default::default())).unwrap();
    loop {
        let event = futures_block(rx.recv()).expect("clone stream should not end silently");
        match event {
            CloneEvent::Progress {
                operation_id: id, ..
            } => assert_eq!(id, operation_id.0),
            CloneEvent::Completed { repo_path, .. } => return PathBuf::from(repo_path),
            CloneEvent::Failed { code, message, .. } => {
                panic!("clone failed ({code}): {message}");
            }
            CloneEvent::Cancelled { .. } => panic!("clone was cancelled unexpectedly"),
        }
    }
}

fn default_branch(repo: &TestRepo) -> String {
    repo.repo.head().unwrap().shorthand().unwrap().to_owned()
}

#[test]
fn local_remote_push_clone_and_pull_round_trip() {
    let seed = TestRepo::init("seed");
    seed.initial_commit(&[("hello.txt", "hello world\n")]);

    let outer = tempfile::tempdir().unwrap();
    let bare_path = outer.path().join("origin.git");
    {
        let mut builder = git2::build::RepoBuilder::new();
        builder.bare(true);
        builder
            .clone(&seed.root.to_string_lossy(), &bare_path)
            .unwrap();
    }

    let backend = Backend::new(BackendConfig::default());
    let opened = futures_block(backend.open_repository(&seed.root)).unwrap();
    let branch = default_branch(&seed);
    let refspec = format!("{branch}:{branch}");

    futures_block(backend.add_remote(
        opened.id,
        RemoteAddRequest {
            name: "origin".into(),
            url: bare_path.to_string_lossy().into_owned(),
            fetch_refspec: None,
        },
        None,
    ))
    .unwrap();

    let remotes = futures_block(backend.list_remotes(opened.id)).unwrap();
    assert!(remotes.iter().any(|r| r.name == "origin"));

    let outcomes = futures_block(backend.push(
        opened.id,
        PushRequest {
            remote: "origin".into(),
            refspecs: vec![refspec.clone()],
            ..Default::default()
        },
        Default::default(),
    ))
    .unwrap();
    assert!(outcomes.iter().all(|o| o.accepted));

    seed.write("second.txt", "second round\n");
    seed.commit_all("second");
    let outcomes2 = futures_block(backend.push(
        opened.id,
        PushRequest {
            remote: "origin".into(),
            refspecs: vec![refspec.clone()],
            ..Default::default()
        },
        Default::default(),
    ))
    .unwrap();
    assert!(outcomes2.iter().all(|o| o.accepted));

    let clone_outer = tempfile::tempdir().unwrap();
    let clone_path = clone_outer.path().join("clone");
    let cloned_root = run_clone_to_completion(
        &backend,
        CloneRequest {
            url: bare_path.to_string_lossy().into_owned(),
            destination: clone_path.to_string_lossy().into_owned(),
            ..Default::default()
        },
    );

    let cloned_opened = futures_block(backend.open_repository(cloned_root.as_path())).unwrap();
    let cloned_status = futures_block(backend.status(
        cloned_opened.id,
        StatusOptions::default(),
        Default::default(),
    ))
    .unwrap();
    assert!(
        cloned_status.is_clean(),
        "fresh clone should be clean, got {:?}",
        cloned_status.entries
    );

    let second_content =
        std::fs::read_to_string(std::path::Path::new(&cloned_root).join("second.txt")).unwrap();
    assert_eq!(
        second_content.trim_end_matches(['\r', '\n']),
        "second round"
    );

    let stale_dir = clone_outer.path().join("stale");
    let stale_root = run_clone_to_completion(
        &backend,
        CloneRequest {
            url: bare_path.to_string_lossy().into_owned(),
            destination: stale_dir.to_string_lossy().into_owned(),
            ..Default::default()
        },
    );

    seed.write("third.txt", "third round\n");
    seed.commit_all("third");
    futures_block(backend.push(
        opened.id,
        PushRequest {
            remote: "origin".into(),
            refspecs: vec![refspec],
            ..Default::default()
        },
        Default::default(),
    ))
    .unwrap();

    let stale_opened = futures_block(backend.open_repository(stale_root.as_path())).unwrap();
    let pulled = futures_block(backend.pull(
        stale_opened.id,
        PullRequest {
            remote: "origin".into(),
            branch: Some(branch),
            fast_forward_only: true,
            rebase: false,
            credential: None,
        },
        None,
        Default::default(),
    ))
    .unwrap();

    assert!(matches!(
        pulled,
        git_backend::engines::git2::workflows::WorkflowOutcome::FastForwarded { .. }
    ));
    assert!(std::path::Path::new(&stale_root).join("third.txt").exists());
}
