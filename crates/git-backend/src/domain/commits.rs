use serde::{Deserialize, Serialize};

use crate::domain::changes::ChangeKind;
use crate::domain::ids::ObjectId;
use crate::domain::paths::RelativePath;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    pub name: String,
    #[serde(default)]
    pub email: String,
    pub time_seconds: i64,
    pub time_offset_minutes: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SearchMatchField {
    Summary,
    Author,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatchRange {
    pub field: SearchMatchField,
    pub start: u32,
    pub length: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitSummary {
    pub id: ObjectId,
    pub tree_id: ObjectId,
    pub parent_ids: Vec<ObjectId>,
    pub summary_line: String,
    pub message: String,
    pub author: Signature,
    pub committer: Signature,
    /// Files touched vs the first parent (root commits diff the empty tree).
    pub files_changed: u32,
    /// Insertions vs the first parent.
    pub additions: u64,
    /// Deletions vs the first parent.
    pub deletions: u64,
    /// Tag names whose target resolves to this commit.
    pub tags: Vec<String>,
    /// Fuzzy-match ranges for the current search, in char offsets of the
    /// displayed `summary_line` / `author.name`. Absent when unsearched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_ranges: Option<Vec<SearchMatchRange>>,
}

impl CommitSummary {
    pub fn is_merge(&self) -> bool {
        self.parent_ids.len() > 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeStat {
    pub path: RelativePath,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<RelativePath>,
    pub kind: ChangeKind,
    pub additions: u32,
    pub deletions: u32,
    pub binary: bool,
    pub old_mode: Option<u32>,
    pub new_mode: Option<u32>,
    pub old_id: Option<ObjectId>,
    pub new_id: Option<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetail {
    pub summary: CommitSummary,
    /// Stats against the first parent; empty for root commits' convenience only.
    pub files: Vec<FileChangeStat>,
    pub total_additions: u64,
    pub total_deletions: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_detection() {
        let oid = || ObjectId::from_bytes(&[5u8; 20]).unwrap();
        let summary = CommitSummary {
            id: oid(),
            tree_id: oid(),
            parent_ids: vec![oid(), oid()],
            summary_line: "Merge branch".into(),
            message: "Merge branch".into(),
            author: sample_signature(),
            committer: sample_signature(),
            files_changed: 0,
            additions: 0,
            deletions: 0,
            tags: vec![],
            match_ranges: None,
        };
        assert!(summary.is_merge());
    }

    fn sample_signature() -> Signature {
        Signature {
            name: "Test".into(),
            email: "test@example.com".into(),
            time_seconds: 1_700_000_000,
            time_offset_minutes: 60,
        }
    }
}
