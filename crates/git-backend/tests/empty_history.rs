mod common;

use common::TestRepo;
use git_backend::api::graph::GraphQuery;
use git_backend::api::history::{HistoryChartQuery, HistoryPageQuery};
use git_backend::domain::RevisionSpec;
use git_backend::runtime::cancellation::CancellationToken;
use git_backend::streaming::model::GraphEvent;
use git_backend::{Backend, BackendConfig};
use std::sync::Arc;

fn backend() -> Arc<Backend> {
    Arc::new(Backend::new(BackendConfig::default()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_page_on_empty_repo_is_empty() {
    let repo = TestRepo::init("empty-history");
    let backend = backend();
    let opened = backend.open_repository(&repo.root).await.unwrap();

    let page = backend
        .history_page(
            opened.id,
            HistoryPageQuery::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(page.commits.is_empty());
    assert!(!page.has_more);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn graph_stream_on_empty_repo_completes_with_no_rows() {
    let repo = TestRepo::init("empty-graph");
    let backend = backend();
    let opened = backend.open_repository(&repo.root).await.unwrap();

    let (_operation_id, mut rx) = backend
        .open_graph(opened.id, GraphQuery::default(), CancellationToken::new())
        .await
        .unwrap();
    let mut rows = 0u64;
    let terminal = loop {
        match rx.recv().await {
            None => break None,
            Some(GraphEvent::Chunk { rows: chunk, .. }) => rows += chunk.len() as u64,
            Some(
                event @ (GraphEvent::Completed { .. }
                | GraphEvent::Failed { .. }
                | GraphEvent::Cancelled { .. }),
            ) => {
                break Some(event);
            }
            Some(_) => {}
        }
    };
    assert!(
        matches!(terminal, Some(GraphEvent::Completed { total_rows: 0, .. })),
        "expected empty Completed, got {terminal:?}"
    );
    assert_eq!(rows, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn history_chart_on_empty_repo_is_empty() {
    let repo = TestRepo::init("empty-chart");
    let backend = backend();
    let opened = backend.open_repository(&repo.root).await.unwrap();

    let chart = backend
        .history_chart(
            opened.id,
            HistoryChartQuery::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(chart.total_commits, 0);
    assert!(chart.buckets.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bogus_revision_still_errors() {
    let repo = TestRepo::init("empty-bogus");
    let backend = backend();
    let opened = backend.open_repository(&repo.root).await.unwrap();

    let query = HistoryPageQuery {
        revision: Some(RevisionSpec::parse("does-not-exist").unwrap()),
        ..HistoryPageQuery::default()
    };
    let error = backend
        .history_page(opened.id, query, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(
        matches!(error, git_backend::error::GitError::InvalidRevision { .. }),
        "expected InvalidRevision, got {error:?}"
    );
}
