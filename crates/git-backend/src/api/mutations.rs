use crate::api::hooks::HookRunResult;
use crate::domain::{CommitSummary, ObjectId, RelativePath, RevisionSpec, Signature};
use serde::{Deserialize, Serialize};

/// A finished commit together with every hook that executed around it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitExecution {
    pub summary: CommitSummary,
    pub hook_runs: Vec<HookRunResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[derive(Default)]
pub struct CommitRequest {
    pub message: String,
    /// Overrides the configured user for this commit.
    pub author: Option<Signature>,
    /// Stage every modification before committing.
    pub stage_all: bool,
    /// Run repository hooks if present (default off).
    pub run_hooks: bool,
    pub allow_empty: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AmendRequest {
    pub message: Option<String>,
    pub author: Option<Signature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BranchCreateRequest {
    pub name: String,
    pub start_point: Option<RevisionSpec>,
    pub force: bool,
    pub checkout: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TagCreateRequest {
    pub name: String,
    pub target: Option<RevisionSpec>,
    pub message: Option<String>,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CheckoutRequest {
    pub target: RevisionSpec,
    pub force: bool,
    /// When set, only restores these paths from the target ("restore").
    pub paths: Vec<String>,
}

impl Default for CheckoutRequest {
    fn default() -> Self {
        Self {
            target: RevisionSpec::head(),
            force: false,
            paths: vec![],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResetKind {
    Soft,
    Mixed,
    Hard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ResetRequest {
    pub kind: ResetKind,
    pub target: RevisionSpec,
}

impl Default for ResetRequest {
    fn default() -> Self {
        Self {
            kind: ResetKind::Mixed,
            target: RevisionSpec::head(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MergeRequest {
    /// Branch or commit to merge into HEAD.
    pub target: RevisionSpec,
    pub fast_forward_only: bool,
    pub no_fast_forward: bool,
    /// Custom merge commit headline.
    pub message: Option<String>,
}

impl Default for MergeRequest {
    fn default() -> Self {
        Self {
            target: RevisionSpec::parse("HEAD").expect("static"),
            fast_forward_only: false,
            no_fast_forward: false,
            message: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MergeContinueRequest {
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MergeAbortRequest {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationKind {
    None,
    Merge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationState {
    pub kind: OperationKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default)]
    pub conflict_paths: Vec<String>,
    #[serde(default)]
    pub heads: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResolveSide {
    Ours,
    Theirs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolveConflictRequest {
    pub path: String,
    pub side: ResolveSide,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictStyle {
    pub light: String,
    pub dark: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub i: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub u: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
    pub path: RelativePath,
    pub stage: u8,
    pub revision: ObjectId,
    pub binary: bool,
    pub rows: Vec<crate::streaming::model::DiffRow>,
    pub styles: Vec<ConflictStyle>,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RevertRequest {
    pub target: RevisionSpec,
    pub parent_index: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StashPushRequest {
    pub message: Option<String>,
    pub include_untracked: bool,
    pub keep_index: bool,
    /// Repository-relative paths. Empty stashes everything; otherwise only
    /// these paths are stashed and reverted.
    #[serde(default)]
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum StashAction {
    #[default]
    Pop,
    Apply,
    Drop,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StashPopRequest {
    pub index: usize,
    pub action: StashAction,
}
