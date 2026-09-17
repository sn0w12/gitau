//! Differential tests pinning the gix diff engine's rows against a git2
//! (libgit2/xdiff) reference computed directly in this file.

mod common;

use common::TestRepo;
use git_backend::api::queries::DiffRequest;
use git_backend::domain::{Generation, OperationId, SnapshotId};
use git_backend::streaming::model::{DiffComparison, DiffEvent, DiffRowKind, SectionKind};
use git_backend::streaming::pipeline::{self, DiffJob};
use git_backend::streaming::store::DiffOperation;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RefSection {
    path: String,
    kind: SectionKind,
    rows: Vec<RefRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RefRow {
    kind: DiffRowKind,
    old_lineno: Option<u32>,
    new_lineno: Option<u32>,
    content: String,
    raw_hex: Option<String>,
}

/// libgit2/xdiff reference: enumerates deltas and materializes patch rows
/// exactly like the previous engine did.
fn reference_sections(root: &std::path::Path, request: &DiffRequest) -> Vec<RefSection> {
    let repo = git2::Repository::discover(root).unwrap();
    let mut opts = git2::DiffOptions::new();
    opts.context_lines(request.context_lines.clamp(0, 64))
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .show_untracked_content(true)
        .skip_binary_check(false);
    let head_tree = {
        repo.head()
            .ok()
            .map(|h| h.peel_to_commit().unwrap())
            .map(|commit| commit.tree().unwrap())
    };
    let diff = match &request.comparison {
        DiffComparison::WorkingTree => {
            repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))
        }
        DiffComparison::Unstaged => repo.diff_index_to_workdir(None, Some(&mut opts)),
        DiffComparison::Staged => {
            repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))
        }
        DiffComparison::TreeToTree { old, new } => {
            let resolve = |spec: &git_backend::domain::RevisionSpec| {
                repo.revparse_single(spec.as_str())
                    .unwrap()
                    .peel_to_tree()
                    .unwrap()
            };
            repo.diff_tree_to_tree(Some(&resolve(old)), Some(&resolve(new)), Some(&mut opts))
        }
        DiffComparison::CommitToParent { commit } => {
            let commit_obj = repo
                .revparse_single(commit.as_str())
                .unwrap()
                .peel_to_commit()
                .unwrap();
            let tree = commit_obj.tree().unwrap();
            match commit_obj.parent(0) {
                Ok(parent) => repo.diff_tree_to_tree(
                    Some(&parent.tree().unwrap()),
                    Some(&tree),
                    Some(&mut opts),
                ),
                Err(_) => repo.diff_tree_to_tree(None, Some(&tree), Some(&mut opts)),
            }
        }
    }
    .unwrap();

    // Rename detection is intentionally not exercised here: it is a known
    // deviation of the gix engine.
    assert!(!request.detect_renames && !request.detect_copies);

    let mut sections = Vec::new();
    for (idx, delta) in diff.deltas().enumerate() {
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let kind = match delta.status() {
            git2::Delta::Added | git2::Delta::Untracked | git2::Delta::Unreadable => {
                SectionKind::Added
            }
            git2::Delta::Deleted => SectionKind::Deleted,
            git2::Delta::Modified | git2::Delta::Ignored => SectionKind::Modified,
            git2::Delta::Renamed => SectionKind::Renamed,
            git2::Delta::Copied => SectionKind::Copied,
            git2::Delta::Typechange => SectionKind::TypeChanged,
            other => panic!("unexpected delta status {other:?}"),
        };

        let mut rows = Vec::new();
        if let Some(patch) = git2::Patch::from_diff(&diff, idx).unwrap() {
            if delta.flags().is_binary()
                || delta.new_file().is_binary()
                || delta.old_file().is_binary()
            {
                let header = format!("diff --git a/{path} b/{path}");
                sections.push(RefSection {
                    path,
                    kind,
                    rows: vec![
                        RefRow {
                            kind: DiffRowKind::FileHeader,
                            old_lineno: None,
                            new_lineno: None,
                            content: header,
                            raw_hex: None,
                        },
                        RefRow {
                            kind: DiffRowKind::BinaryNotice,
                            old_lineno: None,
                            new_lineno: None,
                            content: "Binary file differs".to_owned(),
                            raw_hex: None,
                        },
                    ],
                });
                continue;
            }
            rows.push(RefRow {
                kind: DiffRowKind::FileHeader,
                old_lineno: None,
                new_lineno: None,
                content: format!("diff --git a/{path} b/{path}"),
                raw_hex: None,
            });
            for hunk in 0..patch.num_hunks() {
                let (header, _) = patch.hunk(hunk).unwrap();
                rows.push(RefRow {
                    kind: DiffRowKind::HunkHeader,
                    old_lineno: None,
                    new_lineno: None,
                    content: String::from_utf8_lossy(header.header())
                        .trim_end_matches(['\n', '\r'])
                        .to_owned(),
                    raw_hex: None,
                });
                for line in 0..patch.num_lines_in_hunk(hunk).unwrap() {
                    let l = patch.line_in_hunk(hunk, line).unwrap();
                    let row_kind = match l.origin() {
                        ' ' => Some(DiffRowKind::Context),
                        '+' => Some(DiffRowKind::Addition),
                        '-' => Some(DiffRowKind::Deletion),
                        '=' => Some(DiffRowKind::Context),
                        '>' => Some(DiffRowKind::Addition),
                        '<' => Some(DiffRowKind::Deletion),
                        _ => None,
                    };
                    if let Some(kind) = row_kind {
                        let marker_only = matches!(l.origin(), '=' | '>' | '<');
                        // Mirrors the original engine's decode_bytes:
                        // invalid UTF-8 becomes empty content plus hex.
                        let (content, raw_hex) = if marker_only {
                            ("\\ No newline at end of file".to_owned(), None)
                        } else {
                            match std::str::from_utf8(l.content()) {
                                Ok(text) => (text.trim_end_matches(['\r', '\n']).to_owned(), None),
                                Err(_) => (String::new(), Some(hex(l.content()))),
                            }
                        };
                        rows.push(RefRow {
                            kind,
                            old_lineno: l.old_lineno(),
                            new_lineno: l.new_lineno(),
                            content,
                            raw_hex,
                        });
                    }
                }
            }
        } else {
            rows.push(RefRow {
                kind: DiffRowKind::FileHeader,
                old_lineno: None,
                new_lineno: None,
                content: format!("diff --git a/{path} b/{path}"),
                raw_hex: None,
            });
            rows.push(RefRow {
                kind: DiffRowKind::BinaryNotice,
                old_lineno: None,
                new_lineno: None,
                content: "Binary file differs".to_owned(),
                raw_hex: None,
            });
        }
        sections.push(RefSection { path, kind, rows });
    }
    sections
}

fn engine_sections(root: &TestRepo, request: DiffRequest) -> Vec<RefSection> {
    let op = Arc::new(DiffOperation::new(
        git_backend::domain::RepoId(1),
        OperationId(7),
        Generation(1),
    ));
    let (tx, mut rx) = tokio::sync::mpsc::channel(4096);
    let job = DiffJob::new(
        root.root.clone(),
        request,
        OperationId(7),
        SnapshotId(1),
        Generation(1),
        git_backend::engines::gix::GixSession::discover(&root.root).unwrap(),
    );
    std::thread::spawn(move || pipeline::run(job, op.clone(), tx));
    let events = tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            events.push(event);
        }
        events
    });

    let mut kinds_by_section: Vec<SectionKind> = Vec::new();
    let mut paths_by_section: Vec<String> = Vec::new();
    for event in &events {
        if let DiffEvent::Started { sections, .. } = event {
            paths_by_section = sections
                .iter()
                .map(|s| s.path.as_str().to_owned())
                .collect();
            kinds_by_section = sections.iter().map(|s| s.kind).collect();
        }
    }
    let mut out: Vec<RefSection> = paths_by_section
        .iter()
        .zip(kinds_by_section)
        .map(|(path, kind)| RefSection {
            path: path.clone(),
            kind,
            rows: Vec::new(),
        })
        .collect();
    for event in &events {
        match event {
            DiffEvent::SectionLayout { .. } => {}
            DiffEvent::Chunk {
                section_id, rows, ..
            } => {
                let section = &mut out[*section_id as usize];
                for row in rows.iter() {
                    section.rows.push(RefRow {
                        kind: row.kind.clone(),
                        old_lineno: row.old_lineno,
                        new_lineno: row.new_lineno,
                        content: row.content.clone(),
                        raw_hex: row.raw_hex.clone(),
                    });
                }
            }
            _ => {}
        }
    }
    for (section, path) in out.iter_mut().zip(paths_by_section) {
        section.path = path;
    }
    out
}

fn assert_parity(root: &TestRepo, request: DiffRequest) {
    let expected = reference_sections(&root.root, &request);
    let actual = engine_sections(root, request);
    assert_eq!(
        expected.len(),
        actual.len(),
        "section count mismatch\nexpected={expected:#?}\nactual={actual:#?}"
    );
    for (e, a) in expected.iter().zip(actual.iter()) {
        assert_eq!(e.path, a.path, "path mismatch");
        assert_eq!(e.kind, a.kind, "kind mismatch for {}", e.path);
        assert_eq!(
            e.rows, a.rows,
            "row mismatch for {}\nexpected={expected:#?}\nactual={actual:#?}",
            e.path
        );
    }
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// Simulates a Windows checkout under `core.autocrlf=true`: the index keeps
/// LF while the worktree file is materialized with CRLF. The unstaged diff
/// must compare the index blob against the CRLF-normalized worktree bytes,
/// not the raw file.
#[test]
fn autocrlf_worktree_diff_is_not_full_file() {
    use std::io::Write;

    let repo = TestRepo::init("dp-autocrlf");
    repo.repo
        .config()
        .unwrap()
        .set_str("core.autocrlf", "true")
        .unwrap();
    repo.write("f.txt", "a\nb\nc\n");
    repo.commit_all("commit lf");

    // Re-materialize the worktree file with CRLF, as git checkout would.
    let path = repo.root.join("f.txt");
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(b"a\r\nb\r\nC\r\n").unwrap();

    // Only the `c` line -> `C` change should materialize, not the whole file.
    let actual = engine_sections(&repo, base_request());
    let additions: Vec<_> = actual[0]
        .rows
        .iter()
        .filter(|r| r.kind == DiffRowKind::Addition)
        .map(|r| r.content.clone())
        .collect();
    let deletions: Vec<_> = actual[0]
        .rows
        .iter()
        .filter(|r| r.kind == DiffRowKind::Deletion)
        .map(|r| r.content.clone())
        .collect();
    assert_eq!(additions, vec!["C".to_owned()]);
    assert_eq!(deletions, vec!["c".to_owned()]);
}

fn base_request() -> DiffRequest {
    DiffRequest {
        detect_renames: false,
        detect_copies: false,
        ..DiffRequest::default()
    }
}

#[test]
fn modified_files_match_reference() {
    let repo = TestRepo::init("dp-mod");
    let initial: String = (0..40).map(|i| format!("line {i}\n")).collect();
    repo.initial_commit(&[("a.txt", initial.as_str()), ("b.txt", "keep\n")]);
    let changed: String = (0..40)
        .map(|i| {
            if i == 20 {
                "CHANGED\n".to_owned()
            } else {
                format!("line {i}\n")
            }
        })
        .collect();
    repo.write("a.txt", &changed);
    assert_parity(&repo, base_request());
}

#[test]
fn additions_deletions_and_untracked_match_reference() {
    let repo = TestRepo::init("dp-add-del");
    repo.initial_commit(&[("gone.txt", "delete me\n"), ("stay.txt", "stay\n")]);
    repo.delete("gone.txt");
    repo.write("fresh.txt", "brand new\n");
    repo.write("nested/dir/deep.txt", "deep\n");
    assert_parity(&repo, base_request());
}

#[test]
fn missing_trailing_newline_on_both_sides_matches_reference() {
    let repo = TestRepo::init("dp-nonl");
    repo.initial_commit(&[("f.txt", "one\ntwo\ntail")]);
    repo.write("f.txt", "one\ntwo\nTAIL");
    assert_parity(&repo, base_request());

    // New side gains a newline, old side lacked one.
    repo.write("f.txt", "one\ntwo\ntail\n");
    assert_parity(&repo, base_request());

    // New side loses its newline.
    repo.write("f.txt", "one\ntwo\nTWO");
    assert_parity(&repo, base_request());
}

#[test]
fn crlf_content_matches_reference() {
    let repo = TestRepo::init("dp-crlf");
    repo.initial_commit(&[("f.txt", "a\r\nb\r\nc\r\n")]);
    repo.write("f.txt", "a\r\nB\r\nc\r\n");
    assert_parity(&repo, base_request());
}

#[test]
fn binary_file_shows_notice() {
    let repo = TestRepo::init("dp-binary");
    let bytes: Vec<u8> = std::iter::once(0u8)
        .chain(1u8..=64)
        .chain(std::iter::repeat_n(b'A', 100))
        .collect();
    repo.initial_commit(&[("data.bin", "text\n")]);
    std::fs::write(repo.root.join("data.bin"), &bytes).unwrap();
    assert_parity(&repo, base_request());
}

#[test]
fn empty_transitions_match_reference() {
    let repo = TestRepo::init("dp-empty");
    repo.initial_commit(&[("was-empty.txt", ""), ("had-text.txt", "words\n")]);
    repo.write("was-empty.txt", "now words\n");
    repo.write("had-text.txt", "");
    assert_parity(&repo, base_request());
}

#[test]
fn invalid_utf8_lines_use_raw_hex() {
    let repo = TestRepo::init("dp-utf8");
    repo.initial_commit(&[("f.bin", "ascii\n")]);
    let mutated: &[u8] = b"ascii\n\xff\xfe bad\n";
    std::fs::write(repo.root.join("f.bin"), mutated).unwrap();
    assert_parity(&repo, base_request());
}

#[test]
fn context_line_variations_match_reference() {
    let repo = TestRepo::init("dp-context");
    let initial: String = (0..60).map(|i| format!("l{i}\n")).collect();
    repo.initial_commit(&[("f.txt", initial.as_str())]);
    let changed: String = (0..60)
        .map(|i| {
            if i == 30 {
                "X\n".to_owned()
            } else {
                format!("l{i}\n")
            }
        })
        .collect();
    repo.write("f.txt", &changed);

    for ctx in [0u32, 1, 5] {
        assert_parity(
            &repo,
            DiffRequest {
                context_lines: ctx,
                ..base_request()
            },
        );
    }
}

#[test]
fn staged_and_unstaged_comparisons_match_reference() {
    let repo = TestRepo::init("dp-sides");
    repo.initial_commit(&[("f.txt", "v1\n")]);
    repo.write("f.txt", "v2\n");
    repo.stage("f.txt");
    repo.write("f.txt", "v3\n");

    assert_parity(&repo, base_request()); // WorkingTree combined view
    assert_parity(
        &repo,
        DiffRequest {
            comparison: DiffComparison::Staged,
            ..base_request()
        },
    );
    assert_parity(
        &repo,
        DiffRequest {
            comparison: DiffComparison::Unstaged,
            ..base_request()
        },
    );
}

#[test]
fn staged_diff_on_unborn_head_matches_reference() {
    let repo = TestRepo::init("dp-unborn-staged");
    repo.write("f.txt", "staged before first commit\n");
    repo.stage("f.txt");

    assert_parity(
        &repo,
        DiffRequest {
            comparison: DiffComparison::Staged,
            ..base_request()
        },
    );
}

#[test]
fn tree_to_tree_and_commit_to_parent_match_reference() {
    let repo = TestRepo::init("dp-tree-tree");
    let first = repo.initial_commit(&[("a.txt", "one\ntwo\n")]);
    repo.write("a.txt", "one\nchanged two\nthree\n");
    repo.write("b.txt", "added\n");
    let second = repo.commit_all("second");

    let oid_spec = |oid: git2::Oid| {
        git_backend::domain::RevisionSpec::from_oid(
            git_backend::domain::ObjectId::from_bytes(oid.as_bytes()).unwrap(),
        )
    };
    let request = DiffRequest {
        comparison: DiffComparison::TreeToTree {
            old: oid_spec(first),
            new: oid_spec(second),
        },
        ..base_request()
    };
    assert_parity(&repo, request);

    let request = DiffRequest {
        comparison: DiffComparison::CommitToParent {
            commit: git_backend::domain::RevisionSpec::head(),
        },
        ..base_request()
    };
    assert_parity(&repo, request);

    // Root commit: no parent, so the engine must diff the empty tree.
    let request = DiffRequest {
        comparison: DiffComparison::CommitToParent {
            commit: oid_spec(first),
        },
        ..base_request()
    };
    assert_parity(&repo, request);
}
