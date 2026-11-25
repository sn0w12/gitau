use serde::Deserialize;

use crate::commands::CommandResult;
use crate::state::SharedState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchMarks {
    pub bootstrap_ms: u64,
    pub render_ms: u64,
}

/// Closes the `app.launch` span once the webview has committed its first frame.
#[tauri::command]
pub fn app_launch_ready(state: SharedState<'_>, marks: LaunchMarks) -> CommandResult<()> {
    let mut slot = state
        .launch
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(launch) = slot.take() else {
        return Ok(());
    };
    launch.set_attribute("js_bootstrap_ms", marks.bootstrap_ms.into());
    launch.set_attribute("js_render_ms", marks.render_ms.into());
    Ok(())
}
