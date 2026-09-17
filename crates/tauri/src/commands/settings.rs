use crate::commands::CommandResult;
use crate::settings::{SettingValue, SettingsError, SettingsSnapshot, ValuesSnapshot};
use crate::state::{AppState, SharedState};

/// Schema key whose changes rebuild the merged global excludes file and
/// refresh every open repository.
const GLOBAL_GITIGNORE_KEY: &str = "globalGitignore";
const SYNTAX_THEME_LIGHT_KEY: &str = "syntaxThemeLight";
const SYNTAX_THEME_DARK_KEY: &str = "syntaxThemeDark";

#[tauri::command]
pub fn settings_load(state: SharedState<'_>) -> CommandResult<SettingsSnapshot> {
    Ok(state.settings.snapshot())
}

#[tauri::command]
pub fn settings_set(
    state: SharedState<'_>,
    key: String,
    value: SettingValue,
) -> CommandResult<ValuesSnapshot> {
    let snapshot = state
        .settings
        .set_and_snapshot(&key, value)
        .map_err(to_serialized)?;
    if key == GLOBAL_GITIGNORE_KEY {
        refresh_global_ignore(&state);
    }
    if key == SYNTAX_THEME_LIGHT_KEY || key == SYNTAX_THEME_DARK_KEY {
        apply_syntax_theme(&state);
    }
    Ok(snapshot)
}

/// Rebuilds the merged excludes file from the current setting value and
/// bumps every open repository. Failures only log: the setting itself is
/// already persisted, and the next save or restart retries the apply.
pub(crate) fn refresh_global_ignore(state: &AppState) {
    let content = state
        .settings
        .get(GLOBAL_GITIGNORE_KEY)
        .and_then(|value| value.as_str().map(str::to_owned));
    let (Some(config_dir), Some(content)) = (state.settings.config_dir(), content) else {
        return;
    };
    if let Err(error) = state
        .backend
        .apply_global_ignore_setting(&content, &config_dir)
    {
        log::error!("global gitignore not applied: {error}");
    }
}

/// Re-reads both syntax theme settings and applies them process-wide,
/// bumping every open repository so diffs restream with the new colors.
pub(crate) fn apply_syntax_theme(state: &AppState) {
    let light = state
        .settings
        .get(SYNTAX_THEME_LIGHT_KEY)
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    let dark = state
        .settings
        .get(SYNTAX_THEME_DARK_KEY)
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    state.backend.apply_syntax_theme_setting(&light, &dark);
}

fn to_serialized(error: SettingsError) -> crate::commands::SerializedError {
    crate::commands::SerializedError {
        code: error.code(),
        message: error.to_string(),
        retryable: false,
        detail: error.detail(),
    }
}
