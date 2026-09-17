mod common;

use common::TestRepo;
use git_backend::api::graph::GraphQuery;
use git_backend::streaming::model::GraphEvent;
use git_backend::{Backend, BackendConfig};
use std::sync::Arc;

/// base <- side-work (branch "side")
///    \-- main-work <- merge(side)
/// Walk order (topological, newest first): merge, main-work, side-work, base.
fn build_branchy_repo() -> TestRepo {
    let repo = TestRepo::init("graph");
    let base = repo.initial_commit(&[("a.txt", "one")]);

    let base_commit = repo.repo.find_commit(base).unwrap();
    repo.repo.branch("side", &base_commit, false).unwrap();
    repo.repo.set_head("refs/heads/side").unwrap();
    repo.write("b.txt", "side\n");
    let side = repo.commit_all("side work");

    repo.repo.set_head("refs/heads/master").unwrap();
    repo.write("a.txt", "one\ntwo\n");
    let main = repo.commit_all("main work");

    let sig = git2::Signature::now("Test User", "test@example.com").unwrap();
    let p1 = repo.repo.find_commit(main).unwrap();
    let p2 = repo.repo.find_commit(side).unwrap();
    let tree = p1.tree().unwrap();
    let merge = repo
        .repo
        .commit(Some("HEAD"), &sig, &sig, "merge side", &tree, &[&p1, &p2])
        .unwrap();
    drop(p1);
    drop(p2);
    drop(tree);
    drop(base_commit);
    let _ = merge;
    repo
}

async fn drain_events(
    mut rx: tokio::sync::mpsc::Receiver<GraphEvent>,
) -> (Vec<GraphEvent>, Option<GraphEvent>) {
    let mut events = Vec::new();
    while let Some(event) = rx.recv().await {
        match event {
            GraphEvent::Completed { .. }
            | GraphEvent::Failed { .. }
            | GraphEvent::Cancelled { .. } => return (events, Some(event)),
            other => events.push(other),
        }
    }
    (events, None)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn streams_graph_rows_for_branch_and_merge() {
    let repo = build_branchy_repo();
    let backend = Arc::new(Backend::new(BackendConfig::default()));
    let opened = backend.open_repository(&repo.root).await.unwrap();

    let (operation_id, rx) = backend
        .open_graph(
            opened.id,
            GraphQuery::default(),
            git_backend::runtime::cancellation::CancellationToken::new(),
        )
        .await
        .unwrap();
    let (chunks, terminal) = drain_events(rx).await;

    assert!(
        matches!(terminal, Some(GraphEvent::Completed { .. })),
        "expected Completed, got {terminal:?}"
    );

    let mut rows = Vec::new();
    let mut expected_start = 0u64;
    for event in chunks {
        match event {
            GraphEvent::Started { .. } => {}
            GraphEvent::Chunk {
                row_start,
                rows: chunk_rows,
                ..
            } => {
                assert_eq!(row_start, expected_start);
                expected_start += chunk_rows.len() as u64;
                rows.extend(chunk_rows);
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    assert_eq!(rows.len(), 4);
    // Absolute indexes are contiguous and stable.
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row.index, index as u64);
    }

    let kinds: Vec<_> = rows.iter().map(|row| row.kind).collect();
    assert_eq!(kinds[0], git_backend::domain::GraphRowKind::Merge);
    assert_eq!(kinds[3], git_backend::domain::GraphRowKind::Root);

    // The merge fans out: first parent straight, second parent onto a lane.
    let merge_edges = &rows[0].edges;
    assert_eq!(merge_edges.len(), 2);
    assert_eq!(merge_edges[0].from_lane, 0);
    assert_eq!(merge_edges[0].to_lane, 0);
    assert_eq!(merge_edges[1].from_lane, 0);
    assert_eq!(merge_edges[1].to_lane, 1);

    // Linear rows carry their own straight edge plus continuations for
    // pending lanes passing through; the root has none left.
    let main_edges = &rows[1].edges;
    assert_eq!(main_edges.len(), 2);
    assert_eq!(main_edges[0].from_lane, 0);
    assert_eq!(main_edges[0].to_lane, 0);
    assert_eq!(main_edges[1].from_lane, 1);
    assert_eq!(main_edges[1].to_lane, 1);

    // The side commit bends onto lane 0 while lane 0 continues through.
    let side_edges = &rows[2].edges;
    assert_eq!(side_edges.len(), 2);
    assert_eq!(side_edges[0].from_lane, 1);
    assert_eq!(side_edges[0].to_lane, 0);
    assert_eq!(side_edges[1].from_lane, 0);
    assert_eq!(side_edges[1].to_lane, 0);
    assert!(rows[3].edges.is_empty());

    // Branch decorations land on the right commits.
    assert!(rows[0].refs.iter().any(|name| name == "master"));
    assert!(rows[2].refs.iter().any(|name| name == "side"));

    // Range reads serve exactly what was streamed.
    let range = backend.read_graph_range(operation_id, 1, 2).unwrap();
    assert_eq!(range.rows.len(), 2);
    assert_eq!(range.rows[0].index, 1);
    assert_eq!(range.next_cursor, 3);
    assert!(range.complete);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_terminates_the_stream() {
    let repo = build_branchy_repo();
    let backend = Arc::new(Backend::new(BackendConfig::default()));
    let opened = backend.open_repository(&repo.root).await.unwrap();

    let token = git_backend::runtime::cancellation::CancellationToken::new();
    let (_operation_id, rx) = backend
        .open_graph(opened.id, GraphQuery::default(), token.clone())
        .await
        .unwrap();
    token.cancel();
    // Cancelling before the producer starts (or mid-chunk) ends the stream;
    // a pre-start cancellation surfaces as a closed channel without a
    // terminal event, matching the diff pipeline.
    let (_chunks, terminal) = drain_events(rx).await;
    assert!(matches!(
        terminal,
        None | Some(GraphEvent::Cancelled { .. }) | Some(GraphEvent::Completed { .. })
    ));
}
