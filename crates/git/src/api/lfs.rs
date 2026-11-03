use serde::{Deserialize, Serialize};

/// One tracked path resolved to LFS: the pointer commit stores its sha256
/// oid and byte size; `materialized` is whether the worktree holds the real
/// bytes instead of the pointer text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LfsFileInfo {
    pub path: String,
    pub oid: String,
    pub size: u64,
    /// Worktree holds the real object bytes (already smudged).
    pub materialized: bool,
    /// The object file exists in the repository's LFS object store.
    pub in_store: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LfsStatus {
    pub total: usize,
    pub materialized: usize,
    pub files: Vec<LfsFileInfo>,
}
