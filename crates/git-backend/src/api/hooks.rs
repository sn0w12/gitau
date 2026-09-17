use serde::{Deserialize, Serialize};

/// A commit-lifecycle hook script discovered in the repository hooks
/// directory (`core.hooksPath` aware). Missing standard hooks are not
/// listed; only files that exist on disk appear.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookInfo {
    /// Git hook name without extension, e.g. `pre-commit`.
    pub name: String,
    /// Absolute path of the script that would execute.
    pub path: String,
    pub executable: bool,
}

/// A commit hook script as editable text. Hooks missing on disk come back
/// `exists: false` with empty content, so the editor can create them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookContent {
    /// Git hook name without extension, e.g. `pre-commit`.
    pub hook: String,
    /// Absolute path the script would be written to.
    pub path: String,
    /// Whether a script already exists on disk.
    pub exists: bool,
    pub content: String,
}

/// Outcome of one manual hook run. A non-zero exit is still an Ok result;
/// the frontend decides how to present failures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookRunResult {
    pub hook: String,
    /// Process exit code; `None` when terminated by a signal or the
    /// executable could not be spawned.
    pub exit_code: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}
