use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRepositoryRequest {
    pub path: String,
}

/// Options controlling how status is computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct StatusOptions {
    pub include_ignored: bool,
    pub include_untracked: bool,
    pub recurse_untracked_dirs: bool,
}

impl Default for StatusOptions {
    fn default() -> Self {
        Self {
            include_ignored: false,
            include_untracked: true,
            recurse_untracked_dirs: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[derive(Default)]
pub struct StageRequest {
    /// Paths to stage; empty means stage everything.
    pub paths: Vec<String>,
    pub all: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[derive(Default)]
pub struct DiscardRequest {
    pub paths: Vec<String>,
    pub all: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StageHunkRequest {
    pub path: String,
    /// Index of the hunk inside the working-tree diff for that path.
    pub hunk_index: usize,
}
