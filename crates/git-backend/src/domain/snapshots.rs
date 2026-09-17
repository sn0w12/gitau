use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::domain::ids::{Generation, ObjectId, SnapshotId};
use crate::domain::revisions::BranchName;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoSnapshot {
    pub id: SnapshotId,
    pub generation: Generation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workdir: Option<PathBuf>,
    pub git_dir: PathBuf,
    pub head: HeadState,
    pub sha_kind: crate::domain::ids::ShaKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum HeadState {
    Attached { branch: String, target: ObjectId },
    Detached { target: ObjectId },
    Unborn { branch: String },
}

impl HeadState {
    pub fn branch(&self) -> Option<&str> {
        match self {
            HeadState::Attached { branch, .. } => Some(branch),
            HeadState::Unborn { branch } => Some(branch),
            HeadState::Detached { .. } => None,
        }
    }

    pub fn target(&self) -> Option<ObjectId> {
        match self {
            HeadState::Attached { target, .. } => Some(*target),
            HeadState::Detached { target } => Some(*target),
            HeadState::Unborn { .. } => None,
        }
    }

    pub fn is_unborn(&self) -> bool {
        matches!(self, HeadState::Unborn { .. })
    }
}

impl RepoSnapshot {
    pub fn head_branch(&self) -> Option<BranchName> {
        self.head
            .branch()
            .and_then(|b| BranchName::parse(b, "branch").ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_state_serializes_tagged() {
        let oid = ObjectId::from_bytes(&[3u8; 20]).unwrap();
        let json = serde_json::to_value(HeadState::Detached { target: oid }).unwrap();
        assert_eq!(json["state"], "detached");
        assert_eq!(json["target"], serde_json::json!(oid.hex()));
    }

    #[test]
    fn accessors_work() {
        let attached = HeadState::Attached {
            branch: "main".into(),
            target: ObjectId::from_bytes(&[1u8; 20]).unwrap(),
        };
        assert_eq!(attached.branch(), Some("main"));
        assert!(!attached.is_unborn());
        let unborn = HeadState::Unborn {
            branch: "main".into(),
        };
        assert!(unborn.target().is_none());
        assert!(unborn.is_unborn());
    }
}
