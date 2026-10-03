//! Commit-lifecycle hook discovery and execution.
//!
//! Mirrors git's behavior where feasible: hooks directory resolution is
//! `core.hooksPath` aware, Windows resolves `.exe` / `.cmd` candidates, and
//! POSIX-style scripts (`#!` shebang, no extension) run through a located
//! `sh` because CreateProcess cannot execute them directly ("bad exe
//! format"). Both pipes are read while the hook runs, so a manual run can
//! show output as the script writes it; commit-time runs share the same
//! launcher so both paths behave identically.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use git2::Repository;
use tokio::sync::mpsc::UnboundedSender;

use crate::ansi::LineStream;
use crate::api::hooks::{
    CommitHookEvent, HookContent, HookInfo, HookOutputChunk, HookOutputLine, HookRunResult,
};
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

/// Executes one hook by name, dropping the output as it streams. Spawn
/// failures surface as failed results rather than errors so a stale list
/// entry still produces an inspectable outcome in the checker UI.
pub fn run_commit_hook(repo: &Repository, hook: &str) -> Result<HookRunResult> {
    run_commit_hook_streamed(repo, hook, |_| {})
}

/// Executes one hook, handing `emit` each output line as the process writes
/// it. `emit` is called from the pipe readers, so it must be `Send + Sync`.
/// The returned result repeats the whole run, parsed the same way, for
/// callers that did not stream.
pub fn run_commit_hook_streamed(
    repo: &Repository,
    hook: &str,
    emit: impl Fn(HookOutputChunk) + Send + Sync + 'static,
) -> Result<HookRunResult> {
    match execute(repo, hook, emit)? {
        HookSpawn::Missing => {
            let message = format!("{hook}: not installed");
            Ok(HookRunResult {
                hook: hook.to_owned(),
                exit_code: None,
                success: false,
                stdout: String::new(),
                lines: vec![HookOutputLine {
                    text: message.clone(),
                    spans: Vec::new(),
                }],
                stderr: message,
                duration_ms: 0,
                styles: Vec::new(),
            })
        }
        HookSpawn::Ran(result) => Ok(result),
    }
}

/// Records a hook result when the script exists. The caller decides whether
/// a failure aborts the surrounding operation, so this never inspects
/// `success`.
///
/// With `events` the run is also reported as it happens, the way a manual run
/// streams: a subscriber watching a commit sees the hook the commit is
/// running without having started it. Events go into an unbounded queue whose
/// receiver lives on the async side, so the pipe readers never block and a
/// frontend that went away only loses the events.
pub(crate) fn capture_existing(
    repo: &Repository,
    hook: &str,
    out: &mut Vec<HookRunResult>,
    events: Option<&UnboundedSender<CommitHookEvent>>,
) -> Result<()> {
    // The hook is resolved twice, once here and once in `execute`: a pipeline
    // with no post-commit installed must not announce a hook that never runs.
    if find_hook_file(&hooks_dir(repo)?, hook).is_none() {
        return Ok(());
    }
    if let Some(events) = events {
        let _ = events.send(CommitHookEvent::Started {
            hook: hook.to_owned(),
        });
    }

    // The reader threads need an owned emitter, so the subscriber and the
    // hook name are cloned into the closure rather than borrowed.
    let subscribing = events.cloned();
    let name = hook.to_owned();
    match execute(repo, hook, move |chunk: HookOutputChunk| {
        if let Some(events) = &subscribing {
            let _ = events.send(CommitHookEvent::Line {
                hook: name.clone(),
                text: chunk.line.text,
                spans: chunk.line.spans,
                styles: chunk.styles,
            });
        }
    })? {
        HookSpawn::Missing => Ok(()),
        HookSpawn::Ran(result) => {
            if let Some(events) = events {
                let _ = events.send(CommitHookEvent::Settled {
                    result: result.clone(),
                });
            }
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

pub(crate) fn execute(
    repo: &Repository,
    hook: &str,
    emit: impl Fn(HookOutputChunk) + Send + Sync + 'static,
) -> Result<HookSpawn> {
    ensure_commit_hook(hook)?;

    let dir = hooks_dir(repo)?;
    let Some(path) = find_hook_file(&dir, hook) else {
        return Ok(HookSpawn::Missing);
    };

    let started = Instant::now();
    let mut child = match spawn_hook(repo, &path) {
        Ok(child) => child,
        Err(error) => {
            return Ok(HookSpawn::Ran(failed_result(
                hook,
                started,
                error.to_string(),
            )));
        }
    };

    Ok(HookSpawn::Ran(collect_output(
        &mut child, hook, started, emit,
    )))
}

/// A run that never started: the reason is all the output there is.
fn failed_result(hook: &str, started: Instant, message: String) -> HookRunResult {
    HookRunResult {
        hook: hook.to_owned(),
        exit_code: None,
        success: false,
        stdout: String::new(),
        lines: vec![HookOutputLine {
            text: message.clone(),
            spans: Vec::new(),
        }],
        stderr: message,
        duration_ms: elapsed_ms(started),
        styles: Vec::new(),
    }
}

fn elapsed_ms(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

#[derive(Clone, Copy)]
enum Stream {
    Stdout,
    Stderr,
}

struct HookOutput<F> {
    capture: Mutex<Capture>,
    emit: F,
}

/// One run's output: the raw bytes per stream, plus the parsed lines the
/// frontend renders.
#[derive(Default)]
struct Capture {
    parser: LineStream,
    stdout: String,
    stderr: String,
    lines: Vec<HookOutputLine>,
    read_failed: bool,
}

impl Capture {
    /// Records one line of a stream. `raw` keeps the bytes as the hook wrote
    /// them, `line` is the same text without its terminator, which is what
    /// gets styled and shown.
    fn push(&mut self, stream: Stream, raw: &str, line: &str) -> HookOutputChunk {
        match stream {
            Stream::Stdout => self.stdout.push_str(raw),
            Stream::Stderr => self.stderr.push_str(raw),
        }
        let interned = self.parser.styles().len();
        let parsed = self.parser.push(line);
        let styles = self.parser.styles()[interned..].to_vec();
        let line = HookOutputLine {
            text: parsed.text,
            spans: parsed.spans,
        };
        self.lines.push(line.clone());
        HookOutputChunk { line, styles }
    }

    /// A pipe that died mid-read leaves the run short of output, so the
    /// reason is recorded and the run cannot pass.
    fn note_read_error(&mut self, error: &std::io::Error) {
        self.read_failed = true;
        let message = format!("hook output stream failed: {error}");
        self.stderr.push_str(&message);
        self.stderr.push('\n');
        self.lines.push(HookOutputLine {
            text: message,
            spans: Vec::new(),
        });
    }
}

/// Reads both pipes of a running hook, emitting each line as it completes.
/// Each pipe gets its own reader thread: a hook that fills one pipe while we
/// read only the other would deadlock, which is the hazard `Command::output`
/// avoids by reading them together.
fn collect_output(
    child: &mut Child,
    hook: &str,
    started: Instant,
    emit: impl Fn(HookOutputChunk) + Send + Sync + 'static,
) -> HookRunResult {
    let output = Arc::new(HookOutput {
        capture: Mutex::new(Capture::default()),
        emit,
    });
    let mut readers = Vec::new();
    if let Some(pipe) = child.stdout.take() {
        readers.push(read_stream(Arc::clone(&output), pipe, Stream::Stdout));
    }
    if let Some(pipe) = child.stderr.take() {
        readers.push(read_stream(Arc::clone(&output), pipe, Stream::Stderr));
    }
    for reader in readers {
        let _ = reader.join();
    }
    let status = child.wait();

    let mut capture = output.capture.lock().unwrap();
    let (exit_code, success) = match &status {
        Ok(status) => (status.code(), status.success()),
        Err(error) => {
            capture.note_read_error(error);
            (None, false)
        }
    };
    HookRunResult {
        hook: hook.to_owned(),
        exit_code,
        success: success && !capture.read_failed,
        stdout: std::mem::take(&mut capture.stdout),
        stderr: std::mem::take(&mut capture.stderr),
        duration_ms: elapsed_ms(started),
        lines: std::mem::take(&mut capture.lines),
        styles: capture.parser.styles().to_vec(),
    }
}

fn read_stream<F, R>(
    output: Arc<HookOutput<F>>,
    pipe: R,
    stream: Stream,
) -> std::thread::JoinHandle<()>
where
    F: Fn(HookOutputChunk) + Send + Sync + 'static,
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut reader = BufReader::new(pipe);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) => return,
                Ok(_) => {}
                Err(error) => {
                    output.capture.lock().unwrap().note_read_error(&error);
                    return;
                }
            }
            let raw = String::from_utf8_lossy(&buffer);
            let line = raw.trim_end_matches('\n').trim_end_matches('\r');
            let chunk = output.capture.lock().unwrap().push(stream, &raw, line);
            // Sent outside the lock so a slow consumer cannot stall the
            // reader, and so the two pipes keep draining independently.
            (output.emit)(chunk);
        }
    })
}

/// Builds the spawn for a hook script. Output is piped so it can be read
/// while the hook runs, and stdin is closed so a hook that reads it fails
/// instead of waiting on the terminal, which is what `Command::output` did.
fn hook_command(
    repo: &Repository,
    program: impl AsRef<std::ffi::OsStr>,
    args: &[&Path],
) -> Command {
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(repo.workdir().unwrap_or(Path::new(".")))
        .env("GIT_DIR", repo.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

/// Spawns a hook through an interpreter when direct execution cannot work.
#[cfg(windows)]
fn spawn_hook(repo: &Repository, path: &Path) -> std::io::Result<Child> {
    use std::os::windows::process::CommandExt;

    const BAD_EXE_FORMAT: i32 = 193;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let attempt = |program: &std::ffi::OsStr, args: &[&Path]| {
        let mut command = hook_command(repo, program, args);
        command.creation_flags(CREATE_NO_WINDOW);
        command.spawn()
    };

    match attempt(path.as_os_str(), &[]) {
        Ok(child) => Ok(child),
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
fn spawn_hook(repo: &Repository, path: &Path) -> std::io::Result<Child> {
    // ETXTBSY: the script is open for writing (an editor save, a sync tool,
    // or a racing install landing between write and exec). The writer closes
    // imminently, so retry briefly instead of failing the hook run.
    const TXT_BUSY: i32 = 26;
    const BUSY_ATTEMPTS: u32 = 10;
    let mut busy_retries = 0;
    loop {
        match hook_command(repo, path, &[]).spawn() {
            Err(error)
                if error.raw_os_error() == Some(TXT_BUSY) && busy_retries + 1 < BUSY_ATTEMPTS =>
            {
                busy_retries += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            // ENOEXEC: the script has no shebang. Git re-runs the file through
            // /bin/sh in that case, so mirror it instead of surfacing
            // "Exec format error" for a script git would happily execute.
            Err(error) if error.raw_os_error() == Some(8) => {
                return hook_command(repo, Path::new("/bin/sh"), &[path]).spawn();
            }
            outcome => return outcome,
        }
    }
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
