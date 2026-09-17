use std::sync::Arc;

use crate::domain::{ChangeKind, Generation, OperationId, RelativePath, RevisionSpec, SnapshotId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum DiffComparison {
    /// Working tree vs HEAD (staged + unstaged combined view).
    #[default]
    WorkingTree,
    /// Working tree vs index (unstaged only).
    Unstaged,
    /// Index vs HEAD (staged only).
    Staged,
    /// Two arbitrary revisions.
    TreeToTree {
        old: RevisionSpec,
        new: RevisionSpec,
    },
    /// A commit against its first parent.
    CommitToParent { commit: RevisionSpec },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiffRowKind {
    FileHeader,
    HunkHeader,
    Context,
    Addition,
    Deletion,
    BinaryNotice,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffRow {
    pub kind: DiffRowKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_lineno: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_lineno: Option<u32>,
    pub content: String,
    /// Hex-encoded raw bytes used when the line is not valid UTF-8.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_hex: Option<String>,
    /// Flat `[start, len, styleId]` triples covering the non-plain segments
    /// of `content`; produced by backend syntax highlighting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spans: Option<Vec<u32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SectionKind {
    Added,
    Deleted,
    Modified,
    Renamed,
    Copied,
    TypeChanged,
    Conflicted,
}

impl From<ChangeKind> for SectionKind {
    fn from(kind: ChangeKind) -> Self {
        match kind {
            ChangeKind::Added => SectionKind::Added,
            ChangeKind::Deleted => SectionKind::Deleted,
            ChangeKind::Modified => SectionKind::Modified,
            ChangeKind::Renamed => SectionKind::Renamed,
            ChangeKind::Copied => SectionKind::Copied,
            ChangeKind::TypeChanged => SectionKind::TypeChanged,
            ChangeKind::Conflicted | ChangeKind::Untracked => SectionKind::Modified,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionMeta {
    pub section_id: u32,
    pub path: RelativePath,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<RelativePath>,
    pub kind: SectionKind,
    pub binary: bool,
    pub image: bool,
    /// True while rows are still being computed.
    pub complete: bool,
}

/// A ranged view into a shared section allocation; serializes exactly like
/// the `[DiffRow]` slice it covers.
#[derive(Debug, Clone)]
pub struct SharedRows {
    backing: Arc<[DiffRow]>,
    start: usize,
    len: usize,
}

impl SharedRows {
    pub fn slice(backing: Arc<[DiffRow]>, start: usize, len: usize) -> Self {
        assert!(start + len <= backing.len(), "chunk range out of bounds");
        Self {
            backing,
            start,
            len,
        }
    }

    pub fn whole(backing: Arc<[DiffRow]>) -> Self {
        let len = backing.len();
        Self {
            backing,
            start: 0,
            len,
        }
    }
}

impl std::ops::Deref for SharedRows {
    type Target = [DiffRow];
    fn deref(&self) -> &[DiffRow] {
        &self.backing[self.start..self.start + self.len]
    }
}

impl<'a> IntoIterator for &'a SharedRows {
    type Item = &'a DiffRow;
    type IntoIter = std::slice::Iter<'a, DiffRow>;
    fn into_iter(self) -> Self::IntoIter {
        self.backing[self.start..self.start + self.len].iter()
    }
}

impl Serialize for SharedRows {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.backing[self.start..self.start + self.len].serialize(serializer)
    }
}

/// One resolved syntax-highlighting style covering both app themes. Spans
/// reference entries by their 1-based position in the section's style table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireStyle {
    pub light: String,
    pub dark: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bold: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub italic: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub underline: bool,
}

/// Streaming protocol events. Serialized with a `event` tag for TS discriminated unions.
#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum DiffEvent {
    Started {
        operation_id: OperationId,
        snapshot_id: SnapshotId,
        generation: u64,
        sections: Vec<SectionMeta>,
        estimated_total_rows: u64,
    },
    SectionLayout {
        operation_id: OperationId,
        section_id: u32,
        start_row: u64,
        row_count: u64,
    },
    Chunk {
        operation_id: OperationId,
        section_id: u32,
        row_start: u64,
        /// Shared with the operation's stored rows; serializes as a plain
        /// array on the wire.
        rows: SharedRows,
        /// One entry per row in `rows`; `None` for plain-text rows. Omitted
        /// entirely when the section is not syntax-highlighted.
        #[serde(skip_serializing_if = "Option::is_none")]
        spans_by_row: Option<Vec<Option<Vec<u32>>>>,
        /// Style-table delta for this chunk; spans' third element indexes
        /// into the per-section table these append to.
        #[serde(skip_serializing_if = "Vec::is_empty", default)]
        styles: Vec<WireStyle>,
    },
    LayoutReady {
        operation_id: OperationId,
        total_rows: u64,
    },
    Completed {
        operation_id: OperationId,
        total_rows: u64,
        additions: u64,
        deletions: u64,
        duration_ms: f64,
    },
    Failed {
        operation_id: OperationId,
        code: String,
        message: String,
    },
    Cancelled {
        operation_id: OperationId,
    },
}

/// Stream shape for commit-graph operations. Rows arrive in walk order in
/// fixed-size chunks; absolute row positions are stable.
#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "event",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum GraphEvent {
    Started {
        operation_id: OperationId,
        snapshot_id: SnapshotId,
        generation: Generation,
    },
    Chunk {
        operation_id: OperationId,
        row_start: u64,
        rows: Vec<crate::domain::GraphRow>,
    },
    Completed {
        operation_id: OperationId,
        total_rows: u64,
    },
    Failed {
        operation_id: OperationId,
        code: String,
        message: String,
    },
    Cancelled {
        operation_id: OperationId,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_serialize_with_event_tag() {
        let ev = DiffEvent::Cancelled {
            operation_id: OperationId(1),
        };
        let json = serde_json::to_value(&ev).unwrap();
        assert_eq!(json["event"], "cancelled");

        let chunk = DiffEvent::Chunk {
            operation_id: OperationId(1),
            section_id: 2,
            row_start: 10,
            rows: SharedRows::whole(
                vec![DiffRow {
                    kind: DiffRowKind::Addition,
                    old_lineno: None,
                    new_lineno: Some(3),
                    content: "hi".into(),
                    raw_hex: None,
                    spans: None,
                }]
                .into(),
            ),
            spans_by_row: Some(vec![Some(vec![0, 2, 1])]),
            styles: vec![WireStyle {
                light: "#111111".into(),
                dark: "#eeeeee".into(),
                bold: false,
                italic: false,
                underline: false,
            }],
        };
        let json = serde_json::to_value(&chunk).unwrap();
        assert_eq!(json["event"], "chunk");
        assert_eq!(json["rows"][0]["kind"], "addition");
        assert_eq!(json["rows"][0]["newLineno"], 3);
        assert!(json["rows"][0].get("oldLineno").is_none());
        assert_eq!(json["spansByRow"][0], serde_json::json!([0, 2, 1]));

        let plain = DiffEvent::Chunk {
            operation_id: OperationId(1),
            section_id: 2,
            row_start: 10,
            rows: SharedRows::whole(Vec::new().into()),
            spans_by_row: None,
            styles: Vec::new(),
        };
        let json = serde_json::to_value(&plain).unwrap();
        assert!(json.get("spansByRow").is_none());
        assert!(json.get("styles").is_none());
    }

    #[test]
    fn graph_events_serialize_with_camel_case_fields() {
        let row = crate::domain::GraphRow {
            index: 0,
            id: crate::domain::ObjectId::from_bytes(&[7u8; 20]).unwrap(),
            lane: 1,
            edges: vec![crate::domain::GraphEdge {
                from_lane: 0,
                to_lane: 1,
            }],
            kind: crate::domain::GraphRowKind::Commit,
            summary_line: "tip".into(),
            author_name: "Ada".into(),
            author_email: "ada@example.com".into(),
            time_seconds: 0,
            tags: vec![],
            refs: vec!["main".into()],
        };

        let chunk = GraphEvent::Chunk {
            operation_id: OperationId(9),
            row_start: 2,
            rows: vec![row],
        };
        let json = serde_json::to_value(&chunk).unwrap();
        assert_eq!(json["event"], "chunk");
        assert_eq!(json["operationId"], 9);
        assert_eq!(json["rowStart"], 2);
        // The row's own fields must be camelCase on the wire too.
        assert_eq!(json["rows"][0]["summaryLine"], "tip");
        assert_eq!(json["rows"][0]["authorName"], "Ada");
        assert_eq!(json["rows"][0]["timeSeconds"], 0);

        let started = GraphEvent::Started {
            operation_id: OperationId(1),
            snapshot_id: SnapshotId(1),
            generation: Generation(3),
        };
        let json = serde_json::to_value(&started).unwrap();
        assert_eq!(json["event"], "started");
        assert!(json["operationId"].is_i64());
    }

    #[test]
    fn comparison_defaults_to_working_tree() {
        assert_eq!(
            serde_json::from_value::<DiffComparison>(serde_json::json!(null)).unwrap_or_default(),
            DiffComparison::WorkingTree
        );
    }

    #[test]
    fn section_kind_maps_from_change_kind() {
        assert_eq!(SectionKind::from(ChangeKind::Renamed), SectionKind::Renamed);
        assert_eq!(
            SectionKind::from(ChangeKind::Untracked),
            SectionKind::Modified
        );
    }
}
