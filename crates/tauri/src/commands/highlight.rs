use git_backend::api::highlight::HighlightedSnippet;

use crate::commands::CommandResult;
use crate::state::SharedState;

/// Highlights one markdown code fence with the active theme pair.
/// Pure and infallible: unknown languages stay plain.
#[tauri::command]
pub fn highlight_code(
    state: SharedState<'_>,
    language: String,
    text: String,
) -> CommandResult<HighlightedSnippet> {
    Ok(state.backend.highlight_code(language, text))
}
