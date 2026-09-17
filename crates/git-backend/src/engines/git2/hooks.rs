//! Commit-lifecycle hook discovery and execution.
//!
//! Mirrors git's behavior where feasible: hooks directory resolution is
//! `core.hooksPath` aware, Windows resolves `.exe` / `.cmd` candidates, and
//! POSIX-style scripts (`#!` shebang, no extension) run through a located
//! `sh` because CreateProcess cannot execute them directly ("bad exe
//! format"). Manual runs capture stdio; commit-time runs share the same
//! launcher so both paths behave identically.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use git2::Repository;

use crate::api::hooks::{HookContent, HookInfo, HookRunResult};
use crate::error::{GitError, Result};

/// Hooks that participate in a `git commit` run, in execution order.
pub const COMMIT_HOOK_NAMES: [&str; 4] = [
    "pre-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
];

pub(crate) fn hooks_dir(repo: &Repository) -> Result<PathBuf> {
    let config_path = repo
        .config()?
        .get_string("core.hooksPath")
        .ok()
        .map(PathBuf::from);
    Ok(config_path.unwrap_or_else(|| repo.path().join("hooks")))
}

#[cfg(windows)]
pub(crate) fn candidate_names(hook: &str) -> [String; 3] {
    [
        format!("{hook}.exe"),
        format!("{hook}.cmd"),
        hook.to_owned(),
    ]
}

#[cfg(not(windows))]
pub(crate) fn candidate_names(hook: &str) -> [String; 1] {
    [hook.to_owned()]
}

fn find_hook_file(hooks_dir: &Path, hook: &str) -> Option<PathBuf> {
    candidate_names(hook)
        .iter()
        .map(|candidate| hooks_dir.join(candidate))
        .find(|path| path.is_file())
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|meta| meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    // Candidates resolve through shell associations on Windows; anything
    // found runs when spawned.
    true
}

/// Outcome of resolving a hook script for execution.
pub(crate) enum HookSpawn {
    /// No script installed under any candidate name.
    Missing,
    Ran(HookRunResult),
}

/// Lists commit-pipeline hooks that exist on disk, in execution order.
/// Shell sample stubs (`*.sample`) never match exact names, so they are
/// excluded naturally.
pub fn list_commit_hooks(repo: &Repository) -> Result<Vec<HookInfo>> {
    let dir = hooks_dir(repo)?;
    let mut hooks = Vec::new();
    for name in COMMIT_HOOK_NAMES {
        if let Some(path) = find_hook_file(&dir, name) {
            hooks.push(HookInfo {
                name: name.to_owned(),
                path: path.to_string_lossy().into_owned(),
                executable: is_executable(&path),
            });
        }
    }
    Ok(hooks)
}

/// Executes one hook by name. Spawn failures surface as failed results
/// rather than errors so a stale list entry still produces an inspectable
/// outcome in the checker UI.
pub fn run_commit_hook(repo: &Repository, hook: &str) -> Result<HookRunResult> {
    match execute(repo, hook)? {
        HookSpawn::Missing => Ok(HookRunResult {
            hook: hook.to_owned(),
            exit_code: None,
            success: false,
            stdout: String::new(),
            stderr: format!("{hook}: not installed"),
            duration_ms: 0,
        }),
        HookSpawn::Ran(result) => Ok(result),
    }
}

/// Records a hook result when the script exists. The caller decides whether
/// a failure aborts the surrounding operation, so this never inspects
/// `success`.
pub(crate) fn capture_existing(
    repo: &Repository,
    hook: &str,
    out: &mut Vec<HookRunResult>,
) -> Result<()> {
    match execute(repo, hook)? {
        HookSpawn::Missing => Ok(()),
        HookSpawn::Ran(result) => {
            out.push(result);
            Ok(())
        }
    }
}

fn ensure_commit_hook(hook: &str) -> Result<()> {
    if !COMMIT_HOOK_NAMES.contains(&hook) {
        return Err(GitError::invalid_input(format!(
            "`{hook}` is not a commit-lifecycle hook"
        )));
    }
    Ok(())
}

/// Reads a commit hook script's contents. A missing script reports
/// `exists: false` with empty content so the editor can create it.
pub fn read_commit_hook(repo: &Repository, hook: &str) -> Result<HookContent> {
    ensure_commit_hook(hook)?;
    let dir = hooks_dir(repo)?;
    let found = find_hook_file(&dir, hook);
    let exists = found.is_some();
    let path = found.unwrap_or_else(|| dir.join(hook));
    let content = if exists {
        std::fs::read_to_string(&path)?
    } else {
        String::new()
    };
    Ok(HookContent {
        hook: hook.to_owned(),
        path: path.to_string_lossy().into_owned(),
        exists,
        content,
    })
}

/// Creates or overwrites a commit hook script. The hooks directory is
/// created when missing; a brand-new script gets the executable bit on
/// POSIX so git can run it, while edits preserve the existing mode.
pub fn write_commit_hook(repo: &Repository, hook: &str, content: &str) -> Result<()> {
    ensure_commit_hook(hook)?;
    let dir = hooks_dir(repo)?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(hook);
    #[cfg(unix)]
    let existed = path.exists();
    std::fs::write(&path, content)?;
    #[cfg(unix)]
    if !existed {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(perms.mode() | 0o111);
        std::fs::set_permissions(&path, perms)?;
    }
    Ok(())
}

pub(crate) fn execute(repo: &Repository, hook: &str) -> Result<HookSpawn> {
    ensure_commit_hook(hook)?;

    let dir = hooks_dir(repo)?;
    let Some(path) = find_hook_file(&dir, hook) else {
        return Ok(HookSpawn::Missing);
    };

    let started = Instant::now();
    let output = launch(repo, &path);
    let failed_result = |message: String| HookRunResult {
        hook: hook.to_owned(),
        exit_code: None,
        success: false,
        stdout: String::new(),
        stderr: message,
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    };

    let output = match output {
        Ok(output) => output,
        Err(error) => {
            return Ok(HookSpawn::Ran(failed_result(error.to_string())));
        }
    };

    Ok(HookSpawn::Ran(HookRunResult {
        hook: hook.to_owned(),
        exit_code: output.status.code(),
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    }))
}

/// Spawns a hook through an interpreter when direct execution cannot work.
#[cfg(windows)]
fn launch(repo: &Repository, path: &Path) -> std::io::Result<std::process::Output> {
    use std::os::windows::process::CommandExt;

    const BAD_EXE_FORMAT: i32 = 193;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let attempt = |program: &std::ffi::OsStr, args: &[&Path]| {
        let mut command = Command::new(program);
        command
            .args(args)
            .current_dir(repo.workdir().unwrap_or(Path::new(".")))
            .env("GIT_DIR", repo.path())
            .creation_flags(CREATE_NO_WINDOW);
        command.output()
    };

    match attempt(path.as_os_str(), &[]) {
        Ok(output) => Ok(output),
        Err(error) if error.raw_os_error() == Some(BAD_EXE_FORMAT) => {
            // Not a PE image: route through the interpreter named by the
            // shebang, or fail with a readable reason when none exists.
            let routed = match interpreter_of(path).as_deref() {
                Some("sh" | "bash" | "dash" | "ksh" | "zsh" | "mksh") => find_posix_shell()
                    .map(|found| (found.into_os_string(), vec![posh_path_arg(path)])),
                Some(program) => {
                    which(program).map(|found| (found.into_os_string(), vec![posh_path_arg(path)]))
                }
                None => None,
            };
            match routed {
                Some((program, args)) => {
                    let arg_refs: Vec<&Path> = args.iter().map(|arg| arg.as_path()).collect();
                    attempt(program.as_os_str(), &arg_refs)
                }
                None => Err(std::io::Error::other(
                    "script needs an interpreter that was not found (POSIX hooks require Git for Windows' sh)",
                )),
            }
        }
        Err(error) => Err(error),
    }
}

#[cfg(not(windows))]
fn launch(repo: &Repository, path: &Path) -> std::io::Result<std::process::Output> {
    let mut command = Command::new(path);
    command
        .current_dir(repo.workdir().unwrap_or(Path::new(".")))
        .env("GIT_DIR", repo.path());
    command.output()
}

/// First line interpreter for `#!` scripts: `/bin/sh` becomes "sh",
/// `/usr/bin/env node` becomes "node". Returns None without a shebang.
#[cfg_attr(not(windows), allow(dead_code))]
fn interpreter_of(path: &Path) -> Option<String> {
    let header = std::fs::read(path).ok()?;
    if header.get(..2) != Some(b"#!") {
        return None;
    }
    let line_end = header
        .iter()
        .position(|byte| *byte == b'\n')
        .unwrap_or(header.len());
    let line = String::from_utf8_lossy(&header[2..line_end]);
    let tokens: Vec<&str> = line.trim_end_matches('\r').split_whitespace().collect();
    let target = *tokens.last()?;
    let program = target.rsplit(['/', '\\']).next().unwrap_or(target);
    Some(program.to_owned())
}

/// Locates a POSIX shell for shebang-less-PE scripts: PATH first, then the
/// standard Git for Windows install roots.
#[cfg(windows)]
fn find_posix_shell() -> Option<PathBuf> {
    if let Some(found) = which("sh") {
        return Some(found);
    }
    let roots = [
        std::env::var_os("ProgramW6432"),
        std::env::var_os("ProgramFiles"),
        std::env::var_os("LocalAppData"),
    ];
    roots.into_iter().flatten().find_map(|root| {
        for tail in ["Git\\bin\\sh.exe", "Git\\usr\\bin\\sh.exe"] {
            let candidate = PathBuf::from(&root).join(tail);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    })
}

#[cfg(windows)]
fn which(program: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var).find_map(|dir| {
        let candidate = dir.join(format!("{program}.exe"));
        candidate.is_file().then_some(candidate)
    })
}

/// Forward-slash form; MSYS shells reject some backslash argument shapes.
#[cfg(windows)]
fn posh_path_arg(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().replace('\\', "/"))
}
