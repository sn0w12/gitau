mod common;

use common::TestRepo;
use git_backend::api::history::HistoryPageQuery;
use git_backend::api::queries::DiffRequest;
use git_backend::domain::RevisionSpec;
use git_backend::runtime::cancellation::CancellationToken;
use git_backend::streaming::model::DiffComparison;
use git_backend::streaming::model::DiffEvent;
use git_backend::{Backend, BackendConfig};
use std::time::Duration;

/// Replays the real app sequence around "first history click":
/// worktree diff opens -> user clicks a history entry -> worktree diff is
/// cancelled -> commit diff (CommitToParent) must complete. Repeats for the
/// next entry and revisits the first.
#[tokio::test]
async fn commit_diff_after_cancelled_worktree_diff_completes() {
    let repo = TestRepo::init("history-flow");
    let _c1 = repo.initial_commit(&[("a.txt", "one\ntwo\n")]);
    repo.write("a.txt", "one changed\n");
    repo.write("b.txt", "new file\n");
    let _c2 = repo.commit_all("second");
    repo.delete("b.txt");
    let _c3 = repo.commit_all("third");

    let backend = std::sync::Arc::new(Backend::new(BackendConfig {
        watch_worktree: false,
        ..Default::default()
    }));
    let opened = backend.open_repository(&repo.root).await.unwrap();

    // Left panel: history page loads first.
    let page = backend
        .history_page(opened.id, HistoryPageQuery::default(), Default::default())
        .await
        .unwrap();
    assert_eq!(page.commits.len(), 3);
    let oids: Vec<String> = page.commits.iter().map(|c| c.id.hex()).collect();

    // Baseline worktree diff, cancelled like an unmounted viewer would.
    let token = CancellationToken::new();
    let (_op_wt, mut rx_wt) = backend
        .open_diff(opened.id, DiffRequest::default(), token.clone())
        .await
        .unwrap();
    let _ = rx_wt.recv().await;
    backend.cancel_operation(_op_wt).unwrap();

    for (label, oid_hex) in [
        ("head-entry", oids[0].clone()),
        ("next", oids[1].clone()),
        ("revisit-head", oids[0].clone()),
    ] {
        let token = CancellationToken::new();
        let (_op, mut rx) = backend
            .open_diff(
                opened.id,
                DiffRequest {
                    comparison: DiffComparison::CommitToParent {
                        commit: RevisionSpec::parse(&oid_hex).unwrap(),
                    },
                    detect_renames: true,
                    ..Default::default()
                },
                token,
            )
            .await
            .unwrap();

        let outcome = tokio::time::timeout(Duration::from_secs(5), async {
            let mut terminal = String::new();
            let mut saw_layout = false;
            while let Some(event) = rx.recv().await {
                match &event {
                    DiffEvent::Started { sections, .. } => {
                        terminal = format!("started({})", sections.len());
                    }
                    DiffEvent::SectionLayout { .. } => saw_layout = true,
                    DiffEvent::Completed { .. } => terminal = "completed".into(),
                    DiffEvent::Failed { message, .. } => terminal = format!("failed: {message}"),
                    DiffEvent::Cancelled { .. } => terminal = "cancelled".into(),
                    _ => {}
                }
            }
            (terminal, saw_layout)
        })
        .await;

        match outcome {
            Ok((terminal, saw_layout)) => {
                assert_eq!(terminal, "completed", "{label}: unexpected terminal state");
                assert!(saw_layout, "{label}: never received SectionLayout");
            }
            Err(_) => panic!("{label}: diff timed out after 5s"),
        }
    }
}
