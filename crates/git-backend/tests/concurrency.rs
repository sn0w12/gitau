mod common;

use common::TestRepo;
use git_backend::api::changes::{StageRequest, StatusOptions};
use git_backend::api::history::HistoryPageQuery;
use git_backend::api::mutations::CommitRequest;
use git_backend::api::queries::DiffRequest;
use git_backend::runtime::cancellation::CancellationToken;
use git_backend::streaming::model::DiffEvent;
use git_backend::{Backend, BackendConfig};
use std::sync::Arc;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reads_and_writes_stay_consistent() {
    let repo = TestRepo::init("concurrent");
    for i in 0..30 {
        repo.write(&format!("f{i}.txt"), "v1\n");
    }
    repo.commit_all("base");

    let backend = Arc::new(Backend::new(BackendConfig {
        max_blocking_operations: 8,
        ..Default::default()
    }));
    let opened = backend.open_repository(&repo.root).await.unwrap();

    let mut readers = Vec::new();
    for _ in 0..4 {
        let backend = backend.clone();
        readers.push(tokio::spawn(async move {
            for _ in 0..5 {
                let _ = backend
                    .status(
                        opened.id,
                        StatusOptions::default(),
                        CancellationToken::new(),
                    )
                    .await
                    .unwrap();
                let _ = backend
                    .history_page(
                        opened.id,
                        HistoryPageQuery::default(),
                        CancellationToken::new(),
                    )
                    .await
                    .unwrap();
            }
        }));
    }

    let writer = {
        let backend = backend.clone();
        tokio::spawn(async move {
            for i in 0..3 {
                backend
                    .stage_paths(
                        opened.id,
                        StageRequest {
                            paths: vec![],
                            all: true,
                        },
                        None,
                    )
                    .await
                    .unwrap();
                backend
                    .commit(
                        opened.id,
                        CommitRequest {
                            message: format!("round {i}"),
                            allow_empty: true,
                            stage_all: false,
                            ..Default::default()
                        },
                        None,
                    )
                    .await
                    .unwrap();
            }
        })
    };

    for reader in readers {
        reader.await.unwrap();
    }
    writer.await.unwrap();

    let snapshot = backend.repository_snapshot(opened.id).await.unwrap();
    assert!(snapshot.generation.0 >= 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancelling_diff_delivers_terminal_event() {
    let repo = TestRepo::init("cancel-diff");
    let base: String = (0..200).map(|i| format!("line {i}\n")).collect();
    let mut files: Vec<(String, &str)> = Vec::new();
    for f in 0..40 {
        files.push((format!("file{f}.txt"), base.as_str()));
    }
    let file_refs: Vec<(&str, &str)> = files.iter().map(|(p, c)| (p.as_str(), *c)).collect();
    repo.initial_commit(&file_refs);
    let changed: String = (0..200).map(|i| format!("changed {i}\n")).collect();
    for f in 0..40 {
        repo.write(&format!("file{f}.txt"), &changed);
    }

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();
    let token = CancellationToken::new();
    let (operation_id, mut rx) = backend
        .open_diff(opened.id, DiffRequest::default(), token)
        .await
        .unwrap();

    backend.cancel_operation(operation_id).unwrap();

    let mut terminal_kind = String::new();
    while let Some(event) = rx.recv().await {
        match event {
            DiffEvent::Cancelled { .. } => terminal_kind = "cancelled".into(),
            DiffEvent::Completed { .. } => terminal_kind = "completed".into(),
            DiffEvent::Failed { .. } => terminal_kind = "failed".into(),
            _ => {}
        }
        if !terminal_kind.is_empty() {
            break;
        }
    }
    assert!(
        matches!(terminal_kind.as_str(), "cancelled" | "completed"),
        "diff must end in a terminal state, got {terminal_kind}"
    );

    let range = backend.read_diff_range(operation_id, 0, 10).unwrap();
    assert!(range.complete);
}

#[tokio::test]
async fn unknown_operation_cancel_is_reported() {
    let backend = Backend::new(BackendConfig::default());
    let err = backend
        .cancel_operation(git_backend::domain::OperationId(999))
        .unwrap_err();
    assert_eq!(err.code(), "objectNotFound");
}
