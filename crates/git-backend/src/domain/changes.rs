use serde::{Deserialize, Serialize};

use crate::domain::ids::{Generation, ObjectId, SnapshotId};
use crate::domain::paths::RelativePath;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    Added,
    Deleted,
    Modified,
    TypeChanged,
    Renamed,
    Copied,
    Conflicted,
    Untracked,
}

impl ChangeKind {
    pub fn letter(self) -> char {
        match self {
            ChangeKind::Added => 'A',
            ChangeKind::Deleted => 'D',
            ChangeKind::Modified => 'M',
            ChangeKind::TypeChanged => 'T',
            ChangeKind::Renamed => 'R',
            ChangeKind::Copied => 'C',
            ChangeKind::Conflicted => 'U',
            ChangeKind::Untracked => '?',
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeSide {
    /// Recorded in the index relative to HEAD (staged).
    Index,
    /// Present in the working tree relative to the index (unstaged).
    Worktree,
}

impl ChangeSide {
    pub(crate) fn id_prefix(self) -> &'static str {
        match self {
            ChangeSide::Index => "index",
            ChangeSide::Worktree => "worktree",
        }
    }
}

/// One selectable change. A path can appear twice, once staged and once
/// unstaged, and each row carries its own stable [`StatusEntry::id`] so the
/// UI can select it and request the matching diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEntry {
    /// Stable selector: `index:<path>` or `worktree:<path>`.
    pub id: String,
    pub side: ChangeSide,
    pub path: RelativePath,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<RelativePath>,
    pub kind: ChangeKind,
}

impl StatusEntry {
    pub fn new(
        side: ChangeSide,
        path: RelativePath,
        old_path: Option<RelativePath>,
        kind: ChangeKind,
    ) -> Self {
        Self {
            id: format!("{}:{}", side.id_prefix(), path.as_str()),
            side,
            path,
            old_path,
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictSide {
    pub stage: u8,
    pub id: ObjectId,
    pub mode: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictEntry {
    pub path: RelativePath,
    pub sides: Vec<ConflictSide>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusReport {
    pub snapshot_id: SnapshotId,
    pub generation: Generation,
    pub entries: Vec<StatusEntry>,
    pub conflicts: Vec<ConflictEntry>,
}

impl StatusReport {
    pub fn is_clean(&self) -> bool {
        self.entries.is_empty() && self.conflicts.is_empty()
    }

    pub fn staged_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.side == ChangeSide::Index)
            .count()
    }

    pub fn unstaged_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|e| e.side == ChangeSide::Worktree)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_kind_letters_and_serde() {
        assert_eq!(ChangeKind::Added.letter(), 'A');
        assert_eq!(
            serde_json::to_value(ChangeKind::TypeChanged).unwrap(),
            serde_json::json!("typeChanged")
        );
    }

    #[test]
    fn entry_ids_are_stable_per_side() {
        let path = RelativePath::parse("src/main.rs").unwrap();
        let index = StatusEntry::new(ChangeSide::Index, path.clone(), None, ChangeKind::Modified);
        let worktree = StatusEntry::new(
            ChangeSide::Worktree,
            path.clone(),
            None,
            ChangeKind::Modified,
        );

        assert_eq!(index.id, "index:src/main.rs");
        assert_eq!(worktree.id, "worktree:src/main.rs");
        assert_ne!(index.id, worktree.id);

        let json = serde_json::to_value(&worktree).unwrap();
        assert_eq!(json["id"], "worktree:src/main.rs");
        assert_eq!(json["side"], "worktree");
        assert_eq!(json["kind"], "modified");
    }

    #[test]
    fn clean_report_has_no_entries() {
        let report = StatusReport {
            snapshot_id: SnapshotId(1),
            generation: Generation(1),
            entries: vec![],
            conflicts: vec![],
        };
        assert!(report.is_clean());
        assert_eq!(report.staged_count(), 0);
    }
}
