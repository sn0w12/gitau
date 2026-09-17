use std::path::{Path, PathBuf};

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

/// Resolves `program` to an absolute path the way a terminal would, when
/// the app's own environment cannot find it.
fn resolve_program(program: &str) -> Option<PathBuf> {
    // A path (with separators or an explicit extension) would have spawned
    // directly; only bare names need shell-style resolution.
    if program.contains('/') || program.contains('\\') {
        return None;
    }
    #[cfg(target_os = "windows")]
    {
        resolve_on_windows(program)
    }
    #[cfg(not(target_os = "windows"))]
    {
        resolve_via_login_shell(program)
    }
}

/// Windows: scan PATH appending each PATHEXT entry, mirroring how cmd
/// resolves `code` to `code.cmd`. In-process, so no subprocess and no
/// codepage surprises on non-ASCII paths.
#[cfg(target_os = "windows")]
fn resolve_on_windows(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let pathext: Vec<String> = match std::env::var_os("PATHEXT") {
        Some(raw) => raw
            .to_string_lossy()
            .split(';')
            .filter(|entry| !entry.is_empty())
            .map(|entry| entry.to_owned())
            .collect(),
        None => vec![
            ".COM".to_owned(),
            ".EXE".to_owned(),
            ".BAT".to_owned(),
            ".CMD".to_owned(),
        ],
    };
    for dir in std::env::split_paths(&path) {
        for ext in &pathext {
            let candidate = dir.join(format!("{program}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// macOS/Linux: GUI apps start with a minimal PATH, so ask the user's login
/// shell where the program lives. Interactive login for zsh/bash picks up
/// rc-file PATH exports too.
#[cfg(not(target_os = "windows"))]
fn resolve_via_login_shell(program: &str) -> Option<PathBuf> {
    let shell = std::env::var_os("SHELL")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/bin/sh"));
    let is_interactive_shell = shell.file_name().is_some_and(|name| {
        let name = name.to_string_lossy();
        name.ends_with("zsh") || name.ends_with("bash")
    });
    let flag = if is_interactive_shell { "-lic" } else { "-lc" };
    let output = std::process::Command::new(&shell)
        .arg(flag)
        .arg(format!("command -v {}", shell_single_quote(program)))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let first = stdout.lines().next().unwrap_or_default().trim();
    if first.is_empty() {
        return None;
    }
    Some(PathBuf::from(first))
}

#[cfg(not(target_os = "windows"))]
fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
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

/// Splits a command line into a program and its arguments, honoring
/// double-quoted segments (`"C:\Program Files\code.exe" --reuse-window`).
fn split_command(line: &str) -> (String, Vec<String>) {
    let mut tokens = tokenize(line).into_iter();
    let program = tokens.next().unwrap_or_default();
    (program, tokens.collect())
}

fn tokenize(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for ch in line.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::resolve_program;
    #[cfg(not(target_os = "windows"))]
    use super::shell_single_quote;
    use super::split_command;

    fn split(input: &str) -> (String, Vec<String>) {
        split_command(input)
    }

    #[test]
    fn splits_plain_command_and_args() {
        assert_eq!(
            split("code --reuse-window"),
            ("code".to_owned(), vec!["--reuse-window".to_owned()])
        );
    }

    #[test]
    fn quoted_program_keeps_spaces() {
        assert_eq!(
            split(r#""C:\Program Files\Microsoft VS Code\bin\code.cmd" --reuse-window"#),
            (
                "C:\\Program Files\\Microsoft VS Code\\bin\\code.cmd".to_owned(),
                vec!["--reuse-window".to_owned()]
            )
        );
    }

    #[test]
    fn quoted_arg_keeps_spaces() {
        assert_eq!(
            split(r#"open -a "Visual Studio Code""#),
            (
                "open".to_owned(),
                vec!["-a".to_owned(), "Visual Studio Code".to_owned()]
            )
        );
    }

    #[test]
    fn collapses_whitespace() {
        assert_eq!(
            split("  code    .  "),
            ("code".to_owned(), vec![".".to_owned()])
        );
    }

    #[test]
    fn empty_or_whitespace_yields_no_program() {
        assert_eq!(split("   "), (String::new(), Vec::new()));
        assert_eq!(split(""), (String::new(), Vec::new()));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn single_quotes_escape_embedded_quotes() {
        assert_eq!(shell_single_quote("plain"), "'plain'".to_owned());
        assert_eq!(shell_single_quote("it's"), "'it'\\''s'".to_owned());
    }

    #[test]
    fn resolution_skips_paths_with_separators() {
        assert_eq!(resolve_program(r"C:\tools\code.exe"), None);
        assert_eq!(resolve_program("/usr/local/bin/code"), None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_resolution_does_not_find_nonexistent_programs() {
        assert_eq!(
            resolve_program("gitau_editor_that_does_not_exist_xyz"),
            None
        );
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn login_shell_resolution_finds_standard_programs() {
        let resolved = resolve_program("sh").expect("`sh` must resolve via the login shell");
        assert!(resolved.is_absolute());
        assert_eq!(
            resolve_program("gitau_editor_that_does_not_exist_xyz"),
            None
        );
    }
}
