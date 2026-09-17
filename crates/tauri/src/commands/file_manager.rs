use std::path::{Path, PathBuf};

use crate::commands::{spawn_detached, CommandResult, SerializedError};

/// Errors launching the OS file manager.
#[derive(Debug, thiserror::Error)]
pub enum FileManagerError {
    #[error("could not start the file manager: {source}")]
    LaunchFailed { source: std::io::Error },
}

impl FileManagerError {
    pub fn code(&self) -> &'static str {
        "fileManagerLaunchFailed"
    }
}

/// Reveals a path in the OS file manager: a directory opens in the file
/// manager, a file is revealed (selected) inside its parent. `relative_path`
/// joins onto `path` on the Rust side, mirroring `open_in_editor`.
#[tauri::command]
pub fn reveal_in_file_manager(path: String, relative_path: Option<String>) -> CommandResult<()> {
    let target = match relative_path.as_deref().filter(|value| !value.is_empty()) {
        Some(relative) => PathBuf::from(&path).join(relative),
        None => PathBuf::from(&path),
    };

    // A missing file (deleted since the status ran) falls back to reveal,
    // which still opens its parent directory.
    let is_dir = std::fs::metadata(&target)
        .map(|meta| meta.is_dir())
        .unwrap_or(false);

    let command = reveal_command(&target, is_dir);
    spawn_detached(command)
        .map_err(|source| to_serialized(FileManagerError::LaunchFailed { source }))?;

    Ok(())
}

fn to_serialized(error: FileManagerError) -> SerializedError {
    SerializedError {
        code: error.code(),
        message: error.to_string(),
        retryable: false,
        detail: match &error {
            FileManagerError::LaunchFailed { source } => Some(source.to_string()),
        },
    }
}

#[cfg(target_os = "windows")]
fn reveal_command(target: &Path, is_dir: bool) -> std::process::Command {
    let mut command = std::process::Command::new("explorer");
    if is_dir {
        // Opening the directory shows its contents.
        command.arg(target);
    } else {
        // `/select,` must stay glued to the path: explorer parses the arg
        // itself and chokes when they arrive as separate argv entries.
        command.arg(format!("/select,{}", target.display()));
    }
    command
}

#[cfg(target_os = "macos")]
fn reveal_command(target: &Path, is_dir: bool) -> std::process::Command {
    let mut command = std::process::Command::new("open");
    if is_dir {
        command.arg(target);
    } else {
        // `-R` reveals the file, selecting it in its parent folder.
        command.arg("-R").arg(target);
    }
    command
}

#[cfg(all(unix, not(target_os = "macos")))]
fn reveal_command(target: &Path, is_dir: bool) -> std::process::Command {
    let mut command = std::process::Command::new("xdg-open");
    // There is no portable per-file selection on Linux; opening the parent
    // directory shows the file in the default file manager.
    let arg = if is_dir {
        target
    } else {
        target.parent().unwrap_or(target)
    };
    command.arg(arg);
    command
}

#[cfg(test)]
mod tests {
    use super::reveal_command;
    use std::path::Path;

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_selects_files_and_opens_directories() {
        let file = reveal_command(Path::new(r"C:\repos\a\src\b.ts"), false);
        let args: Vec<_> = file
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, vec![r"/select,C:\repos\a\src\b.ts".to_owned()]);

        let dir = reveal_command(Path::new(r"C:\repos\a"), true);
        let args: Vec<_> = dir
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, vec![r"C:\repos\a".to_owned()]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_reveals_files_and_opens_directories() {
        let file = reveal_command(Path::new("/repos/a/src/b.ts"), false);
        let args: Vec<_> = file
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, vec!["-R".to_owned(), "/repos/a/src/b.ts".to_owned()]);

        let dir = reveal_command(Path::new("/repos/a"), true);
        let args: Vec<_> = dir
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, vec!["/repos/a".to_owned()]);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn linux_opens_directories_and_file_parents() {
        let file = reveal_command(Path::new("/repos/a/src/b.ts"), false);
        let args: Vec<_> = file
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, vec!["/repos/a/src".to_owned()]);

        let dir = reveal_command(Path::new("/repos/a"), true);
        let args: Vec<_> = dir
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, vec!["/repos/a".to_owned()]);
    }
}
