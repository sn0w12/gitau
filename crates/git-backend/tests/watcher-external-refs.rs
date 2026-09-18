mod common;

use common::TestRepo;
use git_backend::api::history::HistoryPageQuery;
use git_backend::runtime::cancellation::CancellationToken;
use git_backend::{Backend, BackendConfig};
use std::time::{Duration, Instant};

/// A tag created outside the app (CLI, script, IDE) only touches
/// `.git/refs/tags/*`. The watcher must surface it: bump the generation so
/// the frontend refetches, and drop the cached tag map so history rows gain
/// the decoration instead of serving the pre-tag summaries.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn external_tag_surfaces_through_watcher() {
    let repo = TestRepo::init("external-tag");
    let head = repo.initial_commit(&[("a.txt", "one\n")]);

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();

    // Prime the history caches while no tag exists.
    let first = backend
        .history_page(
            opened.id,
            HistoryPageQuery::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(first.commits.iter().all(|commit| commit.tags.is_empty()));

    let baseline = backend
        .repository_snapshot(opened.id)
        .await
        .unwrap()
        .generation;

    // Raw git tag, bypassing the backend: no explicit invalidation runs.
    let target = repo.repo.find_commit(head).unwrap();
    repo.repo
        .tag_lightweight("v0.1.3", target.as_object(), false)
        .unwrap();
    drop(target);

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let current = backend
            .repository_snapshot(opened.id)
            .await
            .unwrap()
            .generation;
        if current != baseline {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "watcher did not pick up the external tag"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    let listing = backend
        .list_branches_and_tags(opened.id, CancellationToken::new())
        .await
        .unwrap();
    assert!(
        listing.tags.iter().any(|tag| tag.name.as_str() == "v0.1.3"),
        "tag listing must include the external tag"
    );

    let page = backend
        .history_page(
            opened.id,
            HistoryPageQuery::default(),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    let tagged: Vec<_> = page
        .commits
        .iter()
        .filter(|commit| !commit.tags.is_empty())
        .collect();
    assert_eq!(tagged.len(), 1, "exactly the tagged commit gains tags");
    assert_eq!(tagged[0].tags, vec!["v0.1.3".to_owned()]);
}
