use std::path::{Path, PathBuf};

#[cfg(all(unix, not(target_os = "macos")))]
use crate::commands::launch::resolve_programs_via_login_shell;
use crate::commands::launch::{resolve_program, split_command};
use crate::commands::{spawn_detached, CommandResult, SerializedError};
use crate::settings::SettingValue;
use crate::state::SharedState;

#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("no terminal found; set the terminal command in Settings")]
    NotFound,
    #[error("could not start terminal `{command}`: {source}")]
    LaunchFailed {
        command: String,
        source: std::io::Error,
    },
}

impl TerminalError {
    pub fn code(&self) -> &'static str {
        match self {
            TerminalError::NotFound => "terminalNotFound",
            TerminalError::LaunchFailed { .. } => "terminalLaunchFailed",
        }
    }
}

/// `{dir}` is replaced with the target path; empty `dir_args` means the
/// emulator inherits it.
struct TerminalLauncher {
    program: String,
    args: Vec<String>,
    dir_args: &'static [&'static str],
}

impl TerminalLauncher {
    fn command(&self, dir: &Path) -> std::process::Command {
        let mut command = std::process::Command::new(&self.program);
        command.args(&self.args);
        for fragment in self.dir_args {
            command.arg(fragment.replace("{dir}", &dir.to_string_lossy()));
        }
        if dir.is_dir() {
            command.current_dir(dir);
        }
        command
    }

    fn with_program(&mut self, program: String) -> &mut Self {
        self.program = program;
        self
    }
}

#[tauri::command]
pub async fn open_in_terminal(state: SharedState<'_>, path: String) -> CommandResult<()> {
    let configured = state
        .settings
        .get("terminalCommand")
        .as_ref()
        .and_then(SettingValue::as_str)
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .map(str::to_owned);

    let label = configured
        .clone()
        .unwrap_or_else(|| "the default terminal".to_owned());
    tauri::async_runtime::spawn_blocking(move || launch(configured.as_deref(), &path))
        .await
        .map_err(|error| {
            to_serialized(TerminalError::LaunchFailed {
                command: label,
                source: std::io::Error::other(error.to_string()),
            })
        })?
}

fn launch(configured: Option<&str>, path: &str) -> CommandResult<()> {
    let dir = PathBuf::from(path);

    let mut launcher = match configured {
        Some(command) => {
            let (program, args) = split_command(command);
            TerminalLauncher {
                program,
                args,
                dir_args: &[],
            }
        }
        None => detect_launcher().ok_or_else(|| to_serialized(TerminalError::NotFound))?,
    };

    match spawn_detached(launcher.command(&dir)) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            let Some(resolved) = resolve_program(&launcher.program) else {
                return Err(to_serialized(TerminalError::LaunchFailed {
                    command: launcher.program,
                    source,
                }));
            };
            let retry = launcher
                .with_program(resolved.to_string_lossy().into_owned())
                .command(&dir);
            spawn_detached(retry).map_err(|source| {
                to_serialized(TerminalError::LaunchFailed {
                    command: launcher.program,
                    source,
                })
            })
        }
        Err(source) => Err(to_serialized(TerminalError::LaunchFailed {
            command: launcher.program,
            source,
        })),
    }
}

fn to_serialized(error: TerminalError) -> SerializedError {
    SerializedError {
        code: error.code(),
        message: error.to_string(),
        retryable: false,
        detail: match &error {
            TerminalError::LaunchFailed { source, .. } => Some(source.to_string()),
            TerminalError::NotFound => None,
        },
    }
}

/// Windows never delegates to the built-in console host, so an explicit
/// choice of it reads the same as no choice. A delegation is therefore always
/// Windows Terminal, which does not inherit the directory from the process
/// that launched it and needs it passed as `-d`.
#[cfg(target_os = "windows")]
fn delegated_console() -> Option<TerminalLauncher> {
    let console: String = windows_registry::CURRENT_USER
        .open(r"Console\%%Startup")
        .ok()?
        .get_string("DelegationConsole")
        .ok()?;
    let executable: String = windows_registry::CURRENT_USER
        .open(format!(r"Console\{}", expand_env_vars(&console)))
        .ok()?
        .get_string("DelegateConsole")
        .ok()?;
    let executable = expand_env_vars(&executable);
    Path::new(&executable).is_file().then(|| TerminalLauncher {
        program: executable,
        args: Vec::new(),
        dir_args: &["-d", "{dir}"],
    })
}

#[cfg(target_os = "windows")]
const CANDIDATES: &[(&str, &[&str])] =
    &[("wt", &["-d", "{dir}"]), ("powershell", &[]), ("cmd", &[])];

#[cfg(target_os = "windows")]
fn detect_launcher() -> Option<TerminalLauncher> {
    delegated_console().or_else(|| {
        CANDIDATES
            .iter()
            .find_map(|(program, dir_args)| launcher_on_path(program, dir_args))
    })
}

#[cfg(target_os = "windows")]
fn launcher_on_path(program: &str, dir_args: &'static [&'static str]) -> Option<TerminalLauncher> {
    resolve_program(program).map(|path| TerminalLauncher {
        program: path.to_string_lossy().into_owned(),
        args: Vec::new(),
        dir_args,
    })
}

/// Recorded as REG_EXPAND_SZ, so `%VAR%` references arrive unexpanded.
#[cfg(target_os = "windows")]
fn expand_env_vars(value: &str) -> String {
    let mut expanded = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else {
            break;
        };
        let name = &after[..end];
        expanded.push_str(&rest[..start]);
        match std::env::var(name) {
            Ok(replacement) => expanded.push_str(&replacement),
            Err(_) => expanded.push_str(&format!("%{name}%")),
        }
        rest = &after[end + 1..];
    }
    expanded.push_str(rest);
    expanded
}

#[cfg(target_os = "macos")]
fn detect_launcher() -> Option<TerminalLauncher> {
    let (flag, app) = default_terminal_app()
        .unwrap_or_else(|| ("-b".to_owned(), "com.apple.Terminal".to_owned()));
    Some(TerminalLauncher {
        program: "/usr/bin/open".to_owned(),
        args: vec![flag, app],
        dir_args: &["{dir}"],
    })
}

/// LaunchServices' handler for `public.shell-script`, which is the type
/// Finder opens shell scripts in.
#[cfg(target_os = "macos")]
fn default_terminal_app() -> Option<(String, String)> {
    let output = std::process::Command::new("/usr/bin/defaults")
        .arg("read")
        .arg("com.apple.LaunchServices/com.apple.launchservices.secure")
        .arg("LSHandlerRoleAll")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let plist = String::from_utf8_lossy(&output.stdout);
    let app = plist_string_after(&plist, "public.shell-script")?;
    let flag = if app.contains('/') { "-a" } else { "-b" };
    Some((flag.to_owned(), app))
}

/// Reads past the key rather than at the next sibling, so a handler nested
/// in a sub-dictionary still resolves.
#[cfg(target_os = "macos")]
fn plist_string_after(plist: &str, key: &str) -> Option<String> {
    let after = plist.split_once(&format!("<key>{key}</key>"))?.1;
    let start = after.find("<string>")? + "<string>".len();
    let end = start + after[start..].find("</string>")?;
    let value = after[start..end].trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// `xdg-terminal-exec` leads because it reads the desktop entries'
/// `Terminal=true` preference.
#[cfg(all(unix, not(target_os = "macos")))]
const EMULATORS: &[(&str, &[&str])] = &[
    ("xdg-terminal-exec", &["{dir}"]),
    ("gnome-terminal", &["--working-directory={dir}"]),
    ("konsole", &["--workdir", "{dir}"]),
    ("xfce4-terminal", &["--working-directory={dir}"]),
    ("alacritty", &["--working-directory", "{dir}"]),
    ("kitty", &["--directory", "{dir}"]),
    ("wezterm", &["start", "--cwd", "{dir}"]),
    // foot has no directory flag and reads the one it was started with.
    ("foot", &[]),
    ("xterm", &["-cd", "{dir}"]),
];

#[cfg(all(unix, not(target_os = "macos")))]
fn detect_launcher() -> Option<TerminalLauncher> {
    if let Some(launcher) = terminal_from_env() {
        return Some(launcher);
    }
    let names: Vec<&str> = EMULATORS.iter().map(|(name, _)| *name).collect();
    let program = resolve_programs_via_login_shell(&names)?
        .into_iter()
        .next()?;
    Some(TerminalLauncher {
        dir_args: dir_args_for(&program),
        program,
        args: Vec::new(),
    })
}

/// `command -v` answers with a path, so match on its last segment.
#[cfg(all(unix, not(target_os = "macos")))]
fn dir_args_for(resolved: &str) -> &'static [&'static str] {
    let name = resolved.rsplit('/').next().unwrap_or_default();
    EMULATORS
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map_or(&[], |(_, args)| *args)
}

/// How users and `.dir-locals` files name their emulator.
#[cfg(all(unix, not(target_os = "macos")))]
fn terminal_from_env() -> Option<TerminalLauncher> {
    let value = std::env::var("TERMINAL").ok()?;
    let (program, args) = split_command(value.trim());
    (!program.is_empty()).then_some(TerminalLauncher {
        program,
        args,
        dir_args: &["{dir}"],
    })
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "windows")]
    use super::expand_env_vars;
    #[cfg(target_os = "macos")]
    use super::plist_string_after;
    use super::TerminalLauncher;
    #[cfg(target_os = "windows")]
    use super::CANDIDATES;
    use std::path::{Path, PathBuf};

    fn args_of(command: &std::process::Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn glued_and_separate_directory_arguments_both_expand() {
        let dir = tempdir();
        let glued = TerminalLauncher {
            program: "gnome-terminal".to_owned(),
            args: vec!["--wait".to_owned()],
            dir_args: &["--working-directory={dir}"],
        };
        let command = glued.command(&dir);
        assert_eq!(
            args_of(&command),
            vec![
                "--wait".to_owned(),
                format!("--working-directory={}", dir.display())
            ]
        );
        assert_eq!(command.get_current_dir(), Some(dir.as_path()));

        let separate = TerminalLauncher {
            program: "konsole".to_owned(),
            args: Vec::new(),
            dir_args: &["--workdir", "{dir}"],
        };
        assert_eq!(
            args_of(&separate.command(&dir)),
            vec!["--workdir".to_owned(), dir.display().to_string()]
        );
    }

    #[test]
    fn an_emulator_without_a_directory_flag_only_inherits_it() {
        let dir = tempdir();
        let launcher = TerminalLauncher {
            program: "foot".to_owned(),
            args: Vec::new(),
            dir_args: &[],
        };
        let command = launcher.command(&dir);
        assert!(args_of(&command).is_empty());
        assert_eq!(command.get_current_dir(), Some(dir.as_path()));
    }

    #[test]
    fn a_missing_directory_never_becomes_the_working_directory() {
        let launcher = TerminalLauncher {
            program: "xterm".to_owned(),
            args: Vec::new(),
            dir_args: &["-cd", "{dir}"],
        };
        let missing = Path::new("/gitau-directory-that-does-not-exist");
        assert_eq!(launcher.command(missing).get_current_dir(), None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_terminal_is_told_the_directory_rather_than_inheriting_it() {
        let dir = tempdir();
        let (program, dir_args) = CANDIDATES[0];
        assert_eq!(program, "wt");
        let launcher = TerminalLauncher {
            program: program.to_owned(),
            args: Vec::new(),
            dir_args,
        };
        assert_eq!(
            args_of(&launcher.command(&dir)),
            vec!["-d".to_owned(), dir.display().to_string()]
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn console_hosts_keep_inheriting_the_directory() {
        for (program, dir_args) in &CANDIDATES[1..] {
            assert_eq!(*dir_args, &[] as &[&str], "{program} takes no flag");
        }
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn env_references_expand_and_unknown_ones_survive() {
        std::env::set_var("GITAU_TERMINAL_TEST", "wt.exe");
        assert_eq!(
            expand_env_vars(r"%GITAU_TERMINAL_TEST%"),
            "wt.exe".to_owned()
        );
        assert_eq!(
            expand_env_vars(r"C:\%GITAU_TERMINAL_TEST_UNSET%\wt.exe"),
            r"C:\%GITAU_TERMINAL_TEST_UNSET%\wt.exe".to_owned()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn reads_the_handler_behind_a_launchservices_key() {
        let plist = r#"<plist version="1.0"><dict>
            <key>LSHandlerRoleAll</key>
            <dict>
                <key>public.shell-script</key>
                <string>com.googlecode.iterm2</string>
                <key>public.folder</key>
                <string>com.apple.finder</string>
            </dict>
        </dict></plist>"#;
        assert_eq!(
            plist_string_after(plist, "public.shell-script").as_deref(),
            Some("com.googlecode.iterm2")
        );
        assert_eq!(
            plist_string_after(plist, "public.folder").as_deref(),
            Some("com.apple.finder")
        );
        assert_eq!(plist_string_after(plist, "public.data"), None);
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gitau-terminal-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }
}
