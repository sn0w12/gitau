use std::path::PathBuf;

/// Spawns a fire-and-forget child process. On Unix the child is reaped by a
/// short-lived thread once it exits so no zombie accumulates; on Windows the
/// process handle is released when the `Child` drops.
pub fn spawn_detached(mut command: std::process::Command) -> std::io::Result<()> {
    #[cfg(not(windows))]
    {
        let mut child = command.spawn()?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
    #[cfg(windows)]
    {
        command.spawn()?;
    }
    Ok(())
}

/// Splits a command line into a program and its arguments, honoring
/// double-quoted segments (`"C:\Program Files\code.exe" --reuse-window`).
pub fn split_command(line: &str) -> (String, Vec<String>) {
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

/// Resolves `program` to an absolute path the way a terminal would, when
/// the app's own environment cannot find it.
pub fn resolve_program(program: &str) -> Option<PathBuf> {
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
            .map(str::to_owned)
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
    let stdout = login_shell_command_v(&shell_single_quote(program))?;
    let first = stdout.lines().next().unwrap_or_default().trim();
    (!first.is_empty()).then(|| PathBuf::from(first))
}

/// The paths that exist, in the order given.
#[cfg(all(unix, not(target_os = "macos")))]
pub fn resolve_programs_via_login_shell(programs: &[&str]) -> Option<Vec<String>> {
    let operands = programs
        .iter()
        .map(|program| shell_single_quote(program))
        .collect::<Vec<_>>()
        .join(" ");
    let stdout = login_shell_command_v(&operands)?;
    let found: Vec<String> = stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    (!found.is_empty()).then_some(found)
}

#[cfg(not(target_os = "windows"))]
fn login_shell_command_v(operands: &str) -> Option<String> {
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
        .arg(format!("command -v {operands}"))
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(not(target_os = "windows"))]
pub fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_os = "windows"))]
    use super::shell_single_quote;
    use super::{resolve_program, split_command};

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
