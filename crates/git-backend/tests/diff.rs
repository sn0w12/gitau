mod common;

use common::TestRepo;
use git_backend::api::queries::DiffRequest;
use git_backend::domain::{Generation, OperationId, SnapshotId};
use git_backend::streaming::model::{DiffComparison, DiffEvent, DiffRowKind};
use git_backend::streaming::pipeline::{self, DiffJob};
use git_backend::streaming::store::DiffOperation;
use std::sync::Arc;

fn run_pipeline_to_events(
    repo: &TestRepo,
    request: DiffRequest,
) -> (Arc<DiffOperation>, Vec<DiffEvent>) {
    let op = Arc::new(DiffOperation::new(
        git_backend::domain::RepoId(1),
        OperationId(7),
        Generation(1),
    ));
    let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
    let job = DiffJob::new(
        repo.root.clone(),
        request,
        OperationId(7),
        SnapshotId(1),
        Generation(1),
        git_backend::engines::gix::GixSession::discover(&repo.root).unwrap(),
    );
    let op_for_thread = op.clone();
    std::thread::spawn(move || pipeline::run(job, op_for_thread, tx));
    let events = tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            events.push(event);
        }
        events
    });
    (op, events)
}

#[test]
fn streams_layout_then_chunks_with_stable_positions() {
    let repo = TestRepo::init("diff-basic");
    repo.initial_commit(&[("a.txt", "one\ntwo\nthree\n"), ("b.txt", "alpha\n")]);
    repo.write("a.txt", "one\nTWO changed\nthree\nfour\n");
    repo.delete("b.txt");
    repo.write("c.txt", "brand new file\n");

    let (_op, events) = run_pipeline_to_events(&repo, DiffRequest::default());

    assert!(matches!(events.first(), Some(DiffEvent::Started { .. })));
    assert!(matches!(events.last(), Some(DiffEvent::Completed { .. })));

    let layouts: Vec<(u32, u64, u64)> = events
        .iter()
        .filter_map(|e| match e {
            DiffEvent::SectionLayout {
                section_id,
                start_row,
                row_count,
                ..
            } => Some((*section_id, *start_row, *row_count)),
            _ => None,
        })
        .collect();
    assert!(!layouts.is_empty());
    for window in layouts.windows(2) {
        let (prev_id, prev_start, prev_count) = window[0];
        let (next_id, next_start, _) = window[1];
        assert_eq!(next_id, prev_id + 1);
        assert_eq!(next_start, prev_start + prev_count);
    }

    let total_from_layout = layouts.iter().map(|(_, _, count)| count).sum::<u64>();
    let completed_total = match events.last().unwrap() {
        DiffEvent::Completed { total_rows, .. } => *total_rows,
        other => panic!("expected completed, got {other:?}"),
    };
    assert_eq!(
        completed_total, total_from_layout,
        "layout positions must sum to the final row count"
    );

    let chunks: Vec<&[git_backend::streaming::model::DiffRow]> = events
        .iter()
        .filter_map(|e| match e {
            DiffEvent::Chunk { rows, .. } => Some(&**rows),
            _ => None,
        })
        .collect();
    let chunk_rows: usize = chunks.iter().map(|rows| rows.len()).sum();
    assert_eq!(
        chunk_rows as u64, completed_total,
        "chunks must carry exactly the rows promised by the layout"
    );
}

#[test]
fn started_event_lists_sections_before_content() {
    let repo = TestRepo::init("diff-started");
    repo.initial_commit(&[("only.txt", "line one\n")]);
    repo.write("only.txt", "line one\nline two\n");

    let (_op, events) = run_pipeline_to_events(&repo, DiffRequest::default());
    match &events[0] {
        DiffEvent::Started {
            sections,
            estimated_total_rows,
            ..
        } => {
            assert_eq!(sections.len(), 1);
            assert_eq!(sections[0].path.as_str(), "only.txt");
            assert!(*estimated_total_rows > 0);
        }
        other => panic!("first event must be Started, got {other:?}"),
    }
}

#[test]
fn hunks_and_line_numbers_match_git_semantics() {
    let repo = TestRepo::init("diff-lines");
    repo.initial_commit(&[("code.rs", "a\nb\nc\nd\ne\n")]);
    repo.write("code.rs", "a\nB2\nc\nd\ne\n");

    let (_op, events) = run_pipeline_to_events(&repo, DiffRequest::default());

    let rows: Vec<&git_backend::streaming::model::DiffRow> = events
        .iter()
        .filter_map(|e| match e {
            DiffEvent::Chunk { rows, .. } => Some(rows),
            _ => None,
        })
        .flatten()
        .collect();

    let deletions: Vec<_> = rows
        .iter()
        .filter(|r| r.kind == DiffRowKind::Deletion)
        .collect();
    let additions: Vec<_> = rows
        .iter()
        .filter(|r| r.kind == DiffRowKind::Addition)
        .collect();

    assert_eq!(deletions.len(), 1);
    assert_eq!(additions.len(), 1);
    assert_eq!(deletions[0].old_lineno, Some(2));
    assert_eq!(additions[0].new_lineno, Some(2));
    assert_eq!(deletions[0].content, "b");
    assert_eq!(additions[0].content, "B2");

    assert!(
        rows.iter()
            .any(|r| r.kind == DiffRowKind::HunkHeader && r.content.starts_with("@@"))
    );
}

#[test]
fn tree_to_tree_diff_is_stable() {
    let repo = TestRepo::init("diff-tree-tree");
    let first = repo.initial_commit(&[("f.txt", "v1\n")]);
    repo.write("f.txt", "v2\n");
    let second = repo.commit_all("second");

    let old = git_backend::domain::RevisionSpec::from_oid(
        git_backend::domain::ObjectId::from_bytes(first.as_bytes()).unwrap(),
    );
    let new = git_backend::domain::RevisionSpec::from_oid(
        git_backend::domain::ObjectId::from_bytes(second.as_bytes()).unwrap(),
    );
    let request = DiffRequest {
        comparison: DiffComparison::TreeToTree { old, new },
        ..Default::default()
    };

    let (op, events) = run_pipeline_to_events(&repo, request);
    assert!(matches!(events.last(), Some(DiffEvent::Completed { .. })));
    let (total, additions, deletions) = op.totals();
    assert!(total >= 3);
    assert_eq!(additions, 1);
    assert_eq!(deletions, 1);
}

#[test]
fn root_commit_diff_lists_every_file_against_empty_tree() {
    let repo = TestRepo::init("diff-root-commit");
    let root = repo.initial_commit(&[("a.txt", "one\ntwo\n"), ("b.txt", "alpha\n")]);

    let request = DiffRequest {
        comparison: DiffComparison::CommitToParent {
            commit: git_backend::domain::RevisionSpec::from_oid(
                git_backend::domain::ObjectId::from_bytes(root.as_bytes()).unwrap(),
            ),
        },
        ..Default::default()
    };

    let (op, events) = run_pipeline_to_events(&repo, request);

    let paths: Vec<String> = match &events[0] {
        DiffEvent::Started { sections, .. } => sections
            .iter()
            .map(|s| s.path.as_str().to_owned())
            .collect(),
        other => panic!("first event must be Started, got {other:?}"),
    };
    assert_eq!(paths, vec!["a.txt".to_owned(), "b.txt".to_owned()]);
    assert!(matches!(events.last(), Some(DiffEvent::Completed { .. })));

    let (total, additions, deletions) = op.totals();
    assert!(total > 0);
    assert_eq!(additions, 3);
    assert_eq!(deletions, 0);
}

#[test]
fn range_reads_reconstruct_streamed_content() {
    let repo = TestRepo::init("diff-range");
    let lines: String = (0..500).map(|i| format!("line {i}\n")).collect();
    repo.initial_commit(&[("big.txt", &lines)]);
    let replacement: String = (0..500).map(|i| format!("changed {i}\n")).collect();
    repo.write("big.txt", &replacement);

    let (op, events) = run_pipeline_to_events(&repo, DiffRequest::default());
    assert!(matches!(events.last(), Some(DiffEvent::Completed { .. })));

    let streamed: Vec<&git_backend::streaming::model::DiffRow> = events
        .iter()
        .filter_map(|e| match e {
            DiffEvent::Chunk { rows, .. } => Some(rows),
            _ => None,
        })
        .flatten()
        .collect();

    let mut paged: Vec<git_backend::streaming::model::DiffRow> = Vec::new();
    let mut cursor = 0u64;
    loop {
        let range = op.read_range(cursor, 64).unwrap();
        let done = !range.has_more;
        paged.extend(range.rows.into_iter().map(|r| r.row));
        cursor = range.next_cursor;
        if done {
            break;
        }
    }

    assert_eq!(paged.len(), streamed.len());
    for (paged_row, streamed_row) in paged.iter().zip(streamed.iter()) {
        assert_eq!(*paged_row, **streamed_row);
    }
}
