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

/// Progress of a hook the commit pipeline runs. A commit has no per-hook
/// command of its own, so the frontend learns about these from the commit's
/// channel and can show a hook running that nobody started by hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CommitHookEvent {
    /// The hook is about to run. Sent only for a hook whose script exists, so
    /// a hook that prints nothing still reads as running.
    Started { hook: String },
    /// One line of output, carrying the styles it interned the way a manual
    /// run's [`HookOutputChunk`] does.
    Line {
        hook: String,
        text: String,
        /// Style ids index the table these append to.
        spans: Vec<u32>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        styles: Vec<SnippetStyle>,
    },
    /// The hook finished, with the result a manual run reports.
    Settled { result: HookRunResult },
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

    fn result_of(hook: &str) -> HookRunResult {
        HookRunResult {
            hook: hook.to_owned(),
            exit_code: Some(0),
            success: true,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 3,
            lines: Vec::new(),
            styles: Vec::new(),
        }
    }

    #[test]
    fn a_commit_hook_event_is_tagged_with_its_phase() {
        let json = serde_json::to_value(CommitHookEvent::Started {
            hook: "pre-commit".to_owned(),
        })
        .expect("event serializes");
        assert_eq!(
            json,
            serde_json::json!({ "type": "started", "hook": "pre-commit" })
        );

        let json = serde_json::to_value(CommitHookEvent::Line {
            hook: "pre-commit".to_owned(),
            text: "2 problems".to_owned(),
            spans: vec![0, 10, 1],
            styles: vec![SnippetStyle {
                light: "#cf222e".to_owned(),
                dark: "#ff7b72".to_owned(),
                ..Default::default()
            }],
        })
        .expect("event serializes");
        assert_eq!(json["type"], "line");
        assert_eq!(json["hook"], "pre-commit");
        assert_eq!(json["spans"], serde_json::json!([0, 10, 1]));
        assert_eq!(json["styles"][0]["dark"], "#ff7b72");

        let json = serde_json::to_value(CommitHookEvent::Settled {
            result: result_of("pre-commit"),
        })
        .expect("event serializes");
        assert_eq!(json["type"], "settled");
        assert_eq!(json["result"]["hook"], "pre-commit");

        // A plain line leaves the style delta out entirely, like a chunk.
        let json = serde_json::to_value(CommitHookEvent::Line {
            hook: "pre-commit".to_owned(),
            text: "ok".to_owned(),
            spans: Vec::new(),
            styles: Vec::new(),
        })
        .expect("event serializes");
        assert!(json.get("styles").is_none());
    }
}
