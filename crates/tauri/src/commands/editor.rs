use std::path::{Path, PathBuf};

use crate::commands::launch::{resolve_program, split_command};
use crate::commands::{spawn_detached, CommandResult, SerializedError};
use crate::settings::SettingValue;
use crate::state::SharedState;

/// Errors launching the configured external editor.
#[derive(Debug, thiserror::Error)]
pub enum EditorError {
    #[error("no editor configured; set the editor command in Settings")]
    NotConfigured,
    #[error("could not start editor `{command}`: {source}")]
    LaunchFailed {
        command: String,
        source: std::io::Error,
    },
}

impl EditorError {
    pub fn code(&self) -> &'static str {
        match self {
            EditorError::NotConfigured => "editorNotConfigured",
            EditorError::LaunchFailed { .. } => "editorLaunchFailed",
        }
    }
}

/// Launches the editor configured in `editorCommand` with the given target.
/// `relative_path` joins onto `path` on the Rust side so Windows separators
/// resolve exactly; callers pass a repo root and a repo-relative file path.
///
/// Resolution and spawning run on the blocking pool: a fallback that cannot
/// find the program consults the shell environment, which can take a beat.
#[tauri::command]
pub async fn open_in_editor(
    state: SharedState<'_>,
    path: String,
    relative_path: Option<String>,
) -> CommandResult<()> {
    let editor_value = state.settings.get("editorCommand");
    let editor = editor_value
        .as_ref()
        .and_then(SettingValue::as_str)
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| to_serialized(EditorError::NotConfigured))?;

    let editor_for_error = editor.clone();
    tauri::async_runtime::spawn_blocking(move || launch_editor(&editor, &path, relative_path))
        .await
        .map_err(|error| {
            to_serialized(EditorError::LaunchFailed {
                command: editor_for_error,
                source: std::io::Error::other(error.to_string()),
            })
        })?
}

fn launch_editor(editor: &str, path: &str, relative_path: Option<String>) -> CommandResult<()> {
    let (program, args) = split_command(editor);
    let mut target = PathBuf::from(path);
    if let Some(relative) = relative_path.as_deref().filter(|value| !value.is_empty()) {
        target.push(relative);
    }

    let command = editor_command(&program, &args, &target);
    match spawn_detached(command) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            // The program resolves in a terminal but not in the app's
            // environment: on Windows `code` is `code.cmd`, which
            // CreateProcess never tries for an extensionless name, and
            // macOS/Linux GUI apps start with a minimal PATH. Resolve it the
            // way a shell would and retry.
            match resolve_program(&program) {
                Some(resolved) => {
                    let retry = editor_command(&resolved.to_string_lossy(), &args, &target);
                    spawn_detached(retry).map_err(|source| {
                        to_serialized(EditorError::LaunchFailed {
                            command: editor.to_owned(),
                            source,
                        })
                    })
                }
                None => Err(to_serialized(EditorError::LaunchFailed {
                    command: editor.to_owned(),
                    source,
                })),
            }
        }
        Err(source) => Err(to_serialized(EditorError::LaunchFailed {
            command: editor.to_owned(),
            source,
        })),
    }
}

fn editor_command(program: &str, args: &[String], target: &Path) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    command.args(args).arg(target);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: a console launcher (e.g. `code.cmd`) must not
        // pop a terminal while the editor starts.
        command.creation_flags(0x0800_0000);
    }
    command
}

fn to_serialized(error: EditorError) -> SerializedError {
    SerializedError {
        code: error.code(),
        message: error.to_string(),
        retryable: false,
        detail: match &error {
            EditorError::LaunchFailed { source, .. } => Some(source.to_string()),
            EditorError::NotConfigured => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{editor_command, split_command};
    use std::path::Path;

    #[test]
    fn configured_arguments_precede_the_target() {
        let (program, args) = split_command("code --reuse-window");
        let command = editor_command(&program, &args, Path::new("/repos/a/b.ts"));
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, vec!["--reuse-window", "/repos/a/b.ts"]);
    }
}
