use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    /// Worktree name, unique within the repository. For linked trees this is
    /// the libgit2 admin-dir key; the primary tree is reported as `main`.
    pub name: String,
    pub path: String,
    /// Short branch name checked out in this tree, when attached.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// True for the tree this repository session is bound to.
    pub is_current: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked_by: Option<String>,
    /// True when the tree's checkout directory no longer exists on disk.
    pub is_prunable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct WorktreeCreateRequest {
    pub name: String,
    /// Defaults to `<repo>/<name>` inside the main worktree when absent.
    pub path: Option<String>,
    /// Branch commit the new tree attaches to. A local branch name attaches
    /// the tree to that branch; any other revision detaches at its commit.
    /// When absent, a new local branch named `name` is created at HEAD.
    pub start_point: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct WorktreeRemoveRequest {
    pub name: String,
    /// Delete the checkout directory even when it still exists on disk.
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct WorktreeLockRequest {
    pub name: String,
    pub reason: Option<String>,
}
