use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmoduleInfo {
    pub name: String,
    /// Repository-relative path of the submodule directory.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Commit the nested checkout points at, when initialized.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// Nested repository exists on disk (content available to work in).
    pub initialized: bool,
    /// The superproject worktree materialized this submodule.
    pub checked_out: bool,
    /// The nested worktree has staged/worktree/untracked changes.
    pub dirty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SubmoduleAddRequest {
    /// Remote URL to register and clone from.
    pub url: String,
    /// Repository-relative directory for the submodule.
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct SubmoduleUpdateRequest {
    /// Submodules to update; empty means every configured submodule.
    pub names: Vec<String>,
    /// Recurse into nested submodules of each updated submodule.
    pub recursive: bool,
    /// Initialize (register the URL) before updating.
    pub init: bool,
}
