mod common;

use common::TestRepo;
use git_backend::api::history::{CommitDetailQuery, HistoryPageQuery};
use git_backend::api::mutations::{
    BranchCreateRequest, CheckoutRequest, CommitRequest, TagCreateRequest,
};
use git_backend::domain::RevisionSpec;
use git_backend::{Backend, BackendConfig};

#[tokio::test]
async fn history_pagination_walks_newest_first() {
    let repo = TestRepo::init("history-page");
    for i in 0..7 {
        repo.write(&format!("file-{i}.txt"), &format!("content {i}\n"));
        repo.commit_all(&format!("commit {i}"));
    }

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();

    let page1 = backend
        .history_page(
            opened.id,
            HistoryPageQuery {
                limit: 3,
                ..Default::default()
            },
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(page1.commits.len(), 3);
    assert!(page1.has_more);
    assert_eq!(page1.commits[0].summary_line, "commit 6");
    assert_eq!(page1.commits[2].summary_line, "commit 4");

    let page_last = backend
        .history_page(
            opened.id,
            HistoryPageQuery {
                limit: 100,
                ..Default::default()
            },
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(page_last.commits.len(), 7);
    assert!(!page_last.has_more);
    assert_eq!(page_last.commits.last().unwrap().summary_line, "commit 0");
}

#[tokio::test]
async fn history_search_filters_commits_and_paginates_matches() {
    let repo = TestRepo::init("history-search");
    for i in 0..7 {
        repo.write(&format!("file-{i}.txt"), &format!("content {i}\n"));
        let msg = if i % 3 == 0 {
            format!("refactor security layer {i}")
        } else {
            format!("commit {i}")
        };
        repo.commit_all(&msg);
    }

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();

    // Case-insensitive message match, newest first, paginated over the
    // filtered set so `skip` counts only matching commits.
    let page1 = backend
        .history_page(
            opened.id,
            HistoryPageQuery {
                search: Some("SECURITY".into()),
                limit: 2,
                ..Default::default()
            },
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(page1.commits.len(), 2);
    assert!(page1.has_more);
    assert_eq!(page1.commits[0].summary_line, "refactor security layer 6");
    assert_eq!(page1.commits[1].summary_line, "refactor security layer 3");

    let page2 = backend
        .history_page(
            opened.id,
            HistoryPageQuery {
                search: Some("security".into()),
                limit: 2,
                skip: 2,
                ..Default::default()
            },
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(page2.commits.len(), 1);
    assert!(!page2.has_more);
    assert_eq!(page2.commits[0].summary_line, "refactor security layer 0");

    // Matching also spans the author identity (name and email).
    let author_page = backend
        .history_page(
            opened.id,
            HistoryPageQuery {
                search: Some("test@example".into()),
                limit: 100,
                ..Default::default()
            },
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(author_page.commits.len(), 7);

    // Empty or whitespace search behaves like no filter.
    let empty_search = backend
        .history_page(
            opened.id,
            HistoryPageQuery {
                search: Some("   ".into()),
                limit: 100,
                ..Default::default()
            },
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(empty_search.commits.len(), 7);

    // Fuzzy typo inside one word: "securty" is not a substring of
    // anything, but it fuzzy-matches "security" and only those commits.
    let fuzzy = backend
        .history_page(
            opened.id,
            HistoryPageQuery {
                search: Some("securty".into()),
                limit: 100,
                ..Default::default()
            },
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(fuzzy.commits.len(), 3);
    assert!(
        fuzzy
            .commits
            .iter()
            .all(|c| c.summary_line.contains("security"))
    );

    // Letters scattered across words do not match: "rl6" subsequences
    // across "refactor security layer 6" but matches nothing now.
    let scattered = backend
        .history_page(
            opened.id,
            HistoryPageQuery {
                search: Some("rl6".into()),
                limit: 100,
                ..Default::default()
            },
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(scattered.commits.len(), 0);

    // Multiple whitespace-separated terms all must match.
    let and_page = backend
        .history_page(
            opened.id,
            HistoryPageQuery {
                search: Some("refactor 6".into()),
                limit: 100,
                ..Default::default()
            },
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(and_page.commits.len(), 1);
    assert_eq!(
        and_page.commits[0].summary_line,
        "refactor security layer 6"
    );

    // A term that matches no field excludes every commit.
    let none = backend
        .history_page(
            opened.id,
            HistoryPageQuery {
                search: Some("refactor 99".into()),
                limit: 100,
                ..Default::default()
            },
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(none.commits.len(), 0);
}

#[tokio::test]
async fn history_search_rejects_letters_scattered_across_words() {
    let repo = TestRepo::init("history-search-scatter");
    repo.write("a.txt", "x\n");
    repo.commit_all("Merge branch 'dev' of https://github.com/sn0w12/Akari into dev");
    repo.write("b.txt", "y\n");
    repo.commit_all("add cookie consent banner");

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();

    // "cookie" is a subsequence of the merge message's scattered letters,
    // but only the banner commit contains the word.
    let page = backend
        .history_page(
            opened.id,
            HistoryPageQuery {
                search: Some("cookie".into()),
                limit: 100,
                ..Default::default()
            },
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(page.commits.len(), 1);
    assert_eq!(page.commits[0].summary_line, "add cookie consent banner");
    let ranges = page.commits[0].match_ranges.as_ref().unwrap();
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].start, 4);
    assert_eq!(ranges[0].length, 6);
}

#[tokio::test]
async fn commit_detail_reports_per_file_stats() {
    let repo = TestRepo::init("detail");
    repo.initial_commit(&[("a.txt", "x\n")]);
    repo.write("b.txt", "new\n");
    repo.write("a.txt", "x\ny\n");
    repo.commit_all("add b and grow a");

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();
    let detail = backend
        .commit_detail(
            opened.id,
            CommitDetailQuery {
                revision: RevisionSpec::head(),
                detect_renames: false,
            },
            Default::default(),
        )
        .await
        .unwrap();

    assert_eq!(detail.detail.summary.summary_line, "add b and grow a");
    assert_eq!(detail.detail.files.len(), 2);
    let added_b = detail
        .detail
        .files
        .iter()
        .find(|f| f.path.as_str() == "b.txt")
        .unwrap();
    assert_eq!(added_b.additions, 1);
    let grown_a = detail
        .detail
        .files
        .iter()
        .find(|f| f.path.as_str() == "a.txt")
        .unwrap();
    assert_eq!(grown_a.additions, 1);
}

#[tokio::test]
async fn stage_commit_and_amend_round_trip() {
    let repo = TestRepo::init("stage-commit");
    repo.initial_commit(&[("a.txt", "one\n")]);
    repo.write("a.txt", "one\ntwo\n");

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();

    let commit = backend
        .commit(
            opened.id,
            CommitRequest {
                message: "grow a".into(),
                stage_all: true,
                ..Default::default()
            },
            Some(opened.snapshot.generation),
        )
        .await
        .unwrap();

    assert_eq!(commit.summary.summary_line, "grow a");
    let head = repo.head_commit();
    assert_eq!(
        git_backend::domain::ObjectId::from_bytes(head.id().as_bytes()).unwrap(),
        commit.summary.id
    );

    let amended = backend
        .amend_commit(
            opened.id,
            git_backend::api::mutations::AmendRequest {
                message: Some("grow a properly".into()),
                author: None,
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(amended.summary_line, "grow a properly");
    assert_eq!(repo.head_commit().message().unwrap(), "grow a properly");
}

#[tokio::test]
async fn empty_commit_is_rejected_unless_allowed() {
    let repo = TestRepo::init("empty-commit");
    repo.initial_commit(&[("a.txt", "one\n")]);

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();

    let err = backend
        .commit(
            opened.id,
            CommitRequest {
                message: "nothing".into(),
                stage_all: true,
                allow_empty: false,
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalidInput");

    backend
        .commit(
            opened.id,
            CommitRequest {
                message: "forced".into(),
                stage_all: true,
                allow_empty: true,
                ..Default::default()
            },
            None,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn branches_and_tags_lifecycle() {
    let repo = TestRepo::init("branch-tag");
    repo.initial_commit(&[("a.txt", "one\n")]);

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();

    let branch = backend
        .create_branch(
            opened.id,
            BranchCreateRequest {
                name: "feature/x".into(),
                start_point: None,
                force: false,
                checkout: false,
            },
            None,
        )
        .await
        .unwrap();
    assert!(!branch.is_head);

    let listing = backend
        .list_branches_and_tags(opened.id, Default::default())
        .await
        .unwrap();
    assert!(
        listing
            .branches
            .iter()
            .any(|b| b.name.as_str() == "feature/x")
    );

    // History read before the tag caches summaries without it; the write
    // must drop that cache so a re-read surfaces the tag immediately rather
    // than on the next fresh load.
    let before = backend
        .history_page(opened.id, HistoryPageQuery::default(), Default::default())
        .await
        .unwrap();
    assert!(before.commits[0].tags.is_empty());
    assert!(!before.commits[0].summary_line.is_empty());

    let tag = backend
        .create_tag(
            opened.id,
            TagCreateRequest {
                name: "v1.0.0".into(),
                target: None,
                message: Some("release".into()),
                force: false,
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(tag.message.as_deref(), Some("release"));
    assert!(tag.tag_object.is_some());

    let after = backend
        .history_page(opened.id, HistoryPageQuery::default(), Default::default())
        .await
        .unwrap();
    assert!(
        after.commits[0].tags.iter().any(|t| t == "v1.0.0"),
        "tag should appear immediately after creation"
    );

    backend
        .delete_tag(opened.id, "v1.0.0".into(), None)
        .await
        .unwrap();
    backend
        .delete_branch(opened.id, "feature/x".into(), false, None)
        .await
        .unwrap();
}

#[tokio::test]
async fn checkout_and_restore_paths() {
    let repo = TestRepo::init("checkout-restore");
    repo.initial_commit(&[("a.txt", "v1\n")]);

    let backend = Backend::new(BackendConfig::default());
    let opened = backend.open_repository(&repo.root).await.unwrap();

    repo.write("a.txt", "dirty\n");
    backend
        .checkout(
            opened.id,
            CheckoutRequest {
                target: RevisionSpec::head(),
                force: true,
                paths: vec!["a.txt".into()],
            },
            None,
        )
        .await
        .unwrap();

    assert_eq!(repo.read("a.txt"), "v1\n");
}
