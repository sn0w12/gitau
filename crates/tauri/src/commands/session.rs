use serde_json::Value;

use crate::commands::{to_session_serialized, CommandResult};
use crate::state::SharedState;

/// Loads the persisted app session (tabs, active tab, open repos).
#[tauri::command]
pub fn session_load(state: SharedState<'_>) -> CommandResult<Value> {
    Ok(state.session.load())
}

/// Persists the whole session document. The frontend owns its schema; the
/// backend only enforces the version field.
#[tauri::command]
pub fn session_save(state: SharedState<'_>, doc: Value) -> CommandResult<()> {
    state.session.save(doc).map_err(to_session_serialized)
}
