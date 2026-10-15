mod common;

use common::TestRepo;
use git_backend::api::queries::DiffRequest;
use git_backend::runtime::cancellation::CancellationToken;
use git_backend::streaming::model::{DiffEvent, DiffRowKind};
use git_backend::{Backend, BackendConfig};
use std::collections::HashMap;

fn parse_range(spec: &str) -> Option<(u32, u32)> {
    match spec.split_once(',') {
        Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
        None => Some((spec.parse().ok()?, 1)),
    }
}

fn parse_hunk(content: &str) -> Option<(u32, u32, u32, u32)> {
    let body = content.strip_prefix("@@ -")?;
    let (old, rest) = body.split_once(" +")?;
    let (new, _) = rest.split_once(" @@")?;
    let (old_start, old_count) = parse_range(old)?;
    let (new_start, new_count) = parse_range(new)?;
    Some((old_start, old_count, new_start, new_count))
}

#[tokio::test]
async fn reveals_hidden_context_between_hunks() {
    let mut original = String::new();
    for line in 1..=40 {
        original.push_str(&format!("line {line:02}\n"));
    }
    let repo = TestRepo::init("diff-context");
    repo.initial_commit(&[("a.txt", &original)]);

    let edited = original
        .replace("line 02", "line two")
        .replace("line 39", "line thirty-nine");
    repo.write("a.txt", &edited);

    let backend = Backend::new(BackendConfig {
        watch_worktree: false,
        ..Default::default()
    });
    let opened = backend.open_repository(&repo.root).await.unwrap();
    let (operation, mut rx) = backend
        .open_diff(opened.id, DiffRequest::default(), CancellationToken::new())
        .await
        .unwrap();

    let mut hunks: HashMap<u32, Vec<(u32, u32, u32, u32)>> = HashMap::new();
    let mut old_line_counts: HashMap<u32, u32> = HashMap::new();
    while let Some(event) = rx.recv().await {
        match event {
            DiffEvent::SectionLayout {
                section_id,
                old_line_count,
                ..
            } => {
                old_line_counts.insert(section_id, old_line_count);
            }
            DiffEvent::Chunk {
                section_id, rows, ..
            } => {
                for row in rows.iter() {
                    if row.kind == DiffRowKind::HunkHeader {
                        if let Some(range) = parse_hunk(&row.content) {
                            hunks.entry(section_id).or_default().push(range);
                        }
                    }
                }
            }
            DiffEvent::Failed { message, .. } => panic!("diff failed: {message}"),
            DiffEvent::Completed { .. } | DiffEvent::Cancelled { .. } => break,
            _ => {}
        }
    }

    let (section_id, ranges) = hunks.iter().next().expect("a section with hunks");
    assert_eq!(ranges.len(), 2, "expected two hunks bracketing a gap");
    let (first_old_start, first_old_count, first_new_start, first_new_count) = ranges[0];
    let (second_old_start, ..) = ranges[1];
    assert_eq!(
        old_line_counts.get(section_id).copied(),
        Some(40),
        "layout reports the old file's line count"
    );

    let gap_old_start = first_old_start + first_old_count;
    let gap_new_start = first_new_start + first_new_count;
    assert!(gap_old_start < second_old_start, "hunks left a gap");

    let expansion = backend
        .read_diff_context(operation, *section_id, gap_old_start, gap_new_start, 2)
        .unwrap();
    assert_eq!(expansion.rows.len(), 2);
    assert!(
        expansion
            .rows
            .iter()
            .all(|r| r.kind == DiffRowKind::Context)
    );
    assert_eq!(expansion.rows[0].old_lineno, Some(gap_old_start));
    assert_eq!(expansion.rows[0].new_lineno, Some(gap_new_start));
    assert_eq!(
        expansion.rows[0].content,
        format!("line {gap_old_start:02}")
    );
    assert_eq!(
        expansion.rows[1].content,
        format!("line {:02}", gap_old_start + 1)
    );
    assert!(expansion.more_below);

    let rejected =
        backend.read_diff_context(operation, *section_id, gap_old_start, gap_new_start, 0);
    assert!(rejected.is_err(), "count of zero is rejected");
}
