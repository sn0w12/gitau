use serde::{Deserialize, Serialize};

use crate::api::highlight::SnippetStyle;

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

/// One line of hook output with the ANSI escapes taken out and the colours
/// resolved into `[start, len, styleId]` triples, the span format the
/// frontend renders for diffs and CI logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutputLine {
    pub text: String,
    /// Style ids index [`HookRunResult::styles`].
    pub spans: Vec<u32>,
}

/// One line of a running hook, delivered as the process writes it. Carries
/// the styles it interned so a consumer can keep one growing table instead of
/// the whole run's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutputChunk {
    #[serde(flatten)]
    pub line: HookOutputLine,
    /// Style-table delta; the ids in `line.spans` index the table these
    /// append to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub styles: Vec<SnippetStyle>,
}

/// Outcome of one hook run. A non-zero exit is still an Ok result; the
/// frontend decides how to present failures.
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
    /// Captured output, parsed the way CI logs are, so the frontend renders
    /// the hook's own colours instead of raw escapes.
    pub lines: Vec<HookOutputLine>,
    /// Style table `lines[].spans` index.
    pub styles: Vec<SnippetStyle>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chunk_reaches_the_frontend_flattened() {
        let chunk = HookOutputChunk {
            line: HookOutputLine {
                text: "2 problems".to_owned(),
                spans: vec![0, 10, 1],
            },
            styles: vec![SnippetStyle {
                light: "#cf222e".to_owned(),
                dark: "#ff7b72".to_owned(),
                ..Default::default()
            }],
        };
        let json = serde_json::to_value(&chunk).expect("chunk serializes");
        assert_eq!(json["text"], "2 problems");
        assert_eq!(json["spans"], serde_json::json!([0, 10, 1]));
        assert_eq!(json["styles"][0]["dark"], "#ff7b72");

        // A line that interns no styles leaves the delta out entirely.
        let plain = HookOutputChunk {
            line: HookOutputLine {
                text: "ok".to_owned(),
                spans: Vec::new(),
            },
            styles: Vec::new(),
        };
        let json = serde_json::to_value(&plain).expect("chunk serializes");
        assert!(json.get("styles").is_none());
    }
}
