//! Commit-lifecycle hook discovery and manual execution against real
//! temp repositories.

mod common;

use std::sync::{Arc, Mutex};

use common::TestRepo;
use git_backend::api::hooks::CommitHookEvent;
use git_backend::api::mutations::CommitRequest;
use git_backend::engines::git2::hooks::{
    list_commit_hooks, read_commit_hook, run_commit_hook, run_commit_hook_streamed,
    write_commit_hook,
};
use git_backend::engines::git2::mutations;

fn install_script(repo: &TestRepo, file_name: &str, contents: &str) {
    let path = repo.root.join(".git").join("hooks").join(file_name);
    std::fs::write(&path, contents).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn shell_script(body: &str) -> String {
    format!("#!/bin/sh\n{body}")
}

/// Windows runner guard: shebang routing needs Git for Windows' sh.
#[cfg(windows)]
fn posix_shell_available() -> bool {
    fn which(program: &str) -> bool {
        std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                dir.join(format!("{program}.exe")).is_file() || dir.join(program).is_file()
            })
        })
    }
    if which("sh") {
        return true;
    }
    ["ProgramW6432", "ProgramFiles", "LocalAppData"]
        .iter()
        .filter_map(std::env::var_os)
        .any(|root| {
            ["Git\\bin\\sh.exe", "Git\\usr\\bin\\sh.exe"]
                .iter()
                .any(|tail| std::path::Path::new(&root).join(tail).is_file())
        })
}

#[cfg(not(windows))]
fn posix_shell_available() -> bool {
    true
}

fn sample_exits_three(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(
            path,
            "#!/bin/sh\necho hook-stdout\necho hook-stderr 1>&2\nexit 3\n",
        )
        .unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(windows)]
    {
        std::fs::write(
            path,
            "@echo off\r\necho hook-stdout\r\necho hook-stderr 1>&2\r\nexit /b 3\r\n",
        )
        .unwrap();
    }
}

#[test]
fn fresh_repo_lists_no_hooks() {
    let repo = TestRepo::init("hooks-empty");
    let listed = list_commit_hooks(&repo.repo).unwrap();
    assert!(listed.is_empty(), "samples must not be listed: {listed:?}");
}

#[test]
fn lists_only_commit_pipeline_hooks_in_order() {
    let repo = TestRepo::init("hooks-list");
    let hooks = repo.root.join(".git").join("hooks");
    sample_exits_three(&hooks.join(if cfg!(windows) {
        "commit-msg.cmd"
    } else {
        "commit-msg"
    }));
    // Not part of the commit pipeline even though it exists.
    std::fs::write(hooks.join("pre-push"), "#!/bin/sh\n").unwrap();

    let listed = list_commit_hooks(&repo.repo).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].name, "commit-msg");
    assert!(listed[0].executable);

    let expected_name = if cfg!(windows) {
        "commit-msg.cmd"
    } else {
        "commit-msg"
    };
    assert!(listed[0].path.ends_with(expected_name));
}

#[test]
fn order_follows_commit_pipeline() {
    let repo = TestRepo::init("hooks-order");
    let hooks = repo.root.join(".git").join("hooks");
    let suffix = if cfg!(windows) { ".cmd" } else { "" };
    let ext = |name: &str| format!("{name}{suffix}");
    for name in ["post-commit", "pre-commit"] {
        let bare = hooks.join(ext(name));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::write(&bare, "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(&bare, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        #[cfg(windows)]
        std::fs::write(&bare, "@echo off\r\n").unwrap();
    }

    let names: Vec<String> = list_commit_hooks(&repo.repo)
        .unwrap()
        .into_iter()
        .map(|hook| hook.name)
        .collect();
    assert_eq!(names, ["pre-commit".to_owned(), "post-commit".to_owned()]);
}

#[test]
fn run_captures_output_and_exit_code() {
    let repo = TestRepo::init("hooks-run");
    let hooks = repo.root.join(".git").join("hooks");
    sample_exits_three(&hooks.join(if cfg!(windows) {
        "pre-commit.cmd"
    } else {
        "pre-commit"
    }));

    let result = run_commit_hook(&repo.repo, "pre-commit").unwrap();
    assert_eq!(result.hook, "pre-commit");
    assert!(!result.success);
    assert_eq!(result.exit_code, Some(3));
    assert_eq!(result.stdout.trim_end(), "hook-stdout");
    assert_eq!(result.stderr.trim_end(), "hook-stderr");
    assert!(result.stdout.contains("hook-stdout"));
}

#[test]
fn missing_script_yields_failed_result() {
    let repo = TestRepo::init("hooks-missing");
    let result = run_commit_hook(&repo.repo, "pre-commit").unwrap();
    assert!(!result.success);
    assert_eq!(result.exit_code, None);
    assert!(result.stderr.contains("not installed"));
}

#[test]
fn rejects_non_commit_hook_names() {
    let repo = TestRepo::init("hooks-guard");
    let error = run_commit_hook(&repo.repo, "pre-push").unwrap_err();
    assert!(
        error.to_string().contains("not a commit-lifecycle hook"),
        "unexpected error: {error}"
    );
}

#[test]
fn read_reports_missing_scripts_with_empty_content() {
    let repo = TestRepo::init("hooks-read-missing");
    let content = read_commit_hook(&repo.repo, "pre-commit").unwrap();
    assert!(!content.exists);
    assert_eq!(content.content, "");
    assert!(content.path.ends_with("pre-commit"));
}

#[test]
fn read_returns_existing_script_contents() {
    let repo = TestRepo::init("hooks-read-existing");
    install_script(&repo, "pre-commit", "#!/bin/sh\necho lint\n");
    let content = read_commit_hook(&repo.repo, "pre-commit").unwrap();
    assert!(content.exists);
    assert_eq!(content.content, "#!/bin/sh\necho lint\n");
}

#[test]
fn write_creates_missing_hook_and_makes_it_executable() {
    let repo = TestRepo::init("hooks-write-create");
    write_commit_hook(&repo.repo, "pre-commit", "#!/bin/sh\necho lint\n").unwrap();

    let content = read_commit_hook(&repo.repo, "pre-commit").unwrap();
    assert!(content.exists);
    assert_eq!(content.content, "#!/bin/sh\necho lint\n");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(repo.root.join(".git/hooks/pre-commit"))
            .unwrap()
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "newly created hook must be executable");
    }
}

#[test]
fn write_overwrites_existing_script() {
    let repo = TestRepo::init("hooks-write-overwrite");
    write_commit_hook(&repo.repo, "commit-msg", "old").unwrap();
    write_commit_hook(&repo.repo, "commit-msg", "new content").unwrap();
    let content = read_commit_hook(&repo.repo, "commit-msg").unwrap();
    assert_eq!(content.content, "new content");
}

#[test]
fn write_rejects_non_commit_hook_names() {
    let repo = TestRepo::init("hooks-write-guard");
    let error = write_commit_hook(&repo.repo, "pre-push", "#!/bin/sh\n").unwrap_err();
    assert!(
        error.to_string().contains("not a commit-lifecycle hook"),
        "unexpected error: {error}"
    );
}

#[test]
fn extensionless_posix_script_routes_through_shell() {
    if !posix_shell_available() {
        eprintln!("skipping: no POSIX shell on this host");
        return;
    }
    let repo = TestRepo::init("hooks-posix-route");
    install_script(
        &repo,
        "pre-commit",
        &shell_script("echo posix-ok\nexit 0\n"),
    );

    let result = run_commit_hook(&repo.repo, "pre-commit").unwrap();
    assert!(result.success, "stderr: {}", result.stderr);
    assert!(result.stdout.contains("posix-ok"));
}

#[test]
fn env_interpreter_scripts_run_via_path_lookup() {
    fn which(program: &str) -> bool {
        std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let bare = dir.join(program);
                bare.is_file() || std::path::Path::new(&format!("{}.exe", bare.display())).is_file()
            })
        })
    }
    if !which("node") {
        eprintln!("skipping: node not on PATH");
        return;
    }
    let repo = TestRepo::init("hooks-node");
    install_script(
        &repo,
        "pre-commit",
        "#!/usr/bin/env node\nprocess.stdout.write(\"node-ok\");\n",
    );

    let result = run_commit_hook(&repo.repo, "pre-commit").unwrap();
    assert!(result.success, "stderr: {}", result.stderr);
    assert!(result.stdout.contains("node-ok"));
}

fn commit_request(message: &str) -> CommitRequest {
    CommitRequest {
        message: message.to_owned(),
        author: None,
        stage_all: false,
        run_hooks: true,
        allow_empty: false,
    }
}

#[test]
fn failing_pre_commit_gates_the_commit_with_details() {
    let repo = TestRepo::init("hooks-gate");
    repo.initial_commit(&[("a.txt", "one\n")]);
    repo.write("a.txt", "one\ntwo\n");
    repo.stage("a.txt");

    let suffix = if cfg!(windows) { ".cmd" } else { "" };
    if cfg!(windows) {
        install_script(
            &repo,
            "pre-commit.cmd",
            "@echo off\r\necho gate-closed\r\nexit /b 5\r\n",
        );
    } else {
        install_script(&repo, "pre-commit", "echo gate-closed\nexit 5\n");
    }

    let before = repo.head_commit().id();
    let error = mutations::commit(&repo.repo, &commit_request("blocked"), None).unwrap_err();
    let rendered = error.to_string();
    assert!(rendered.contains("gate-closed"), "details: {rendered}");
    // Windows batch exits surface the numeric code through runHook; unix
    // exit(5) appears verbatim.
    let _ = suffix;

    let after = repo.head_commit().id();
    assert_eq!(before, after, "commit must be blocked");
}

#[test]
fn successful_commit_collects_pipeline_results_in_order() {
    if !posix_shell_available() {
        eprintln!("skipping: no POSIX shell on this host");
        return;
    }
    let repo = TestRepo::init("hooks-natural");
    repo.initial_commit(&[("a.txt", "one\n")]);
    repo.write("a.txt", "one\ntwo\n");
    repo.stage("a.txt");

    install_script(&repo, "pre-commit", &shell_script("echo pre-ran\n"));
    install_script(&repo, "post-commit", &shell_script("echo post-ran\n"));

    let (summary, runs) =
        mutations::commit(&repo.repo, &commit_request("with hooks"), None).unwrap();
    assert_eq!(summary.summary_line, "with hooks");
    let names: Vec<&str> = runs.iter().map(|run| run.hook.as_str()).collect();
    assert_eq!(names, ["pre-commit", "post-commit"]);
    assert!(runs.iter().all(|run| run.success));
}

#[test]
fn a_commit_reports_each_hook_it_runs_as_it_runs() {
    if !posix_shell_available() {
        eprintln!("skipping: no POSIX shell on this host");
        return;
    }
    let repo = TestRepo::init("hooks-commit-stream");
    repo.initial_commit(&[("a.txt", "one\n")]);
    repo.write("a.txt", "one\ntwo\n");
    repo.stage("a.txt");

    install_script(&repo, "pre-commit", &shell_script("echo checking\n"));
    install_script(&repo, "post-commit", &shell_script("echo posted\n"));

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    mutations::commit(&repo.repo, &commit_request("watched"), Some(sender)).unwrap();

    // Everything queued before the commit returned has to be waiting here, so
    // a frontend that watches the commit sees each hook start and settle.
    let mut seen: Vec<CommitHookEvent> = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        seen.push(event);
    }
    let phases: Vec<String> = seen
        .iter()
        .map(|event| match event {
            CommitHookEvent::Started { hook } => format!("start {hook}"),
            CommitHookEvent::Line { hook, text, .. } => format!("line {hook} {text}"),
            CommitHookEvent::Settled { result } => format!("settled {} ok", result.success),
        })
        .collect();
    assert_eq!(
        phases,
        [
            "start pre-commit",
            "line pre-commit checking",
            "settled true ok",
            "start post-commit",
            "line post-commit posted",
            "settled true ok",
        ]
    );
}

#[test]
fn a_gated_commit_still_reports_the_hook_that_blocked_it() {
    if !posix_shell_available() {
        eprintln!("skipping: no POSIX shell on this host");
        return;
    }
    let repo = TestRepo::init("hooks-commit-gate-stream");
    repo.initial_commit(&[("a.txt", "one\n")]);
    repo.write("a.txt", "one\ntwo\n");
    repo.stage("a.txt");
    install_script(
        &repo,
        "pre-commit",
        &shell_script("echo gate-closed\nexit 5\n"),
    );

    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    mutations::commit(&repo.repo, &commit_request("blocked"), Some(sender)).unwrap_err();

    // The commit fails, but the failing hook's output still reaches the
    // subscriber: its result is otherwise lost in the error's details string.
    let mut settled = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        if let CommitHookEvent::Settled { result } = event {
            settled.push(result);
        }
    }
    assert_eq!(settled.len(), 1);
    assert_eq!(settled[0].hook, "pre-commit");
    assert!(!settled[0].success);
    assert!(
        settled[0]
            .lines
            .iter()
            .any(|line| line.text == "gate-closed")
    );
}

#[test]
fn a_commit_without_a_subscriber_still_runs_its_hooks() {
    if !posix_shell_available() {
        eprintln!("skipping: no POSIX shell on this host");
        return;
    }
    let repo = TestRepo::init("hooks-commit-silent");
    repo.initial_commit(&[("a.txt", "one\n")]);
    repo.write("a.txt", "one\ntwo\n");
    repo.stage("a.txt");
    install_script(&repo, "pre-commit", &shell_script("echo ran\n"));

    let (_summary, runs) =
        mutations::commit(&repo.repo, &commit_request("unwatched"), None).unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].lines[0].text, "ran");
}

#[test]
fn a_streamed_run_reports_each_line_with_its_colour_resolved() {
    if !posix_shell_available() {
        eprintln!("skipping: no POSIX shell on this host");
        return;
    }
    let repo = TestRepo::init("hooks-stream");
    install_script(
        &repo,
        "pre-commit",
        &shell_script("printf '\\033[32mformatted 2 files\\033[0m\\nplain line\\n'"),
    );

    let sink = Arc::new(Mutex::new(Vec::new()));
    let received = Arc::clone(&sink);
    let result = run_commit_hook_streamed(&repo.repo, "pre-commit", move |chunk| {
        received.lock().unwrap().push(chunk);
    })
    .unwrap();

    let streamed = sink.lock().unwrap();
    let texts: Vec<&str> = streamed
        .iter()
        .map(|chunk| chunk.line.text.as_str())
        .collect();
    assert_eq!(texts, ["formatted 2 files", "plain line"]);
    // The escape is gone from the text and became a span.
    assert_eq!(streamed[0].line.spans, vec![0, 17, 1]);
    assert!(streamed[1].line.spans.is_empty());
    let interned: usize = streamed.iter().map(|chunk| chunk.styles.len()).sum();
    assert_eq!(interned, 1, "the green is interned once for the run");

    // The settled result repeats the stream, parsed the same way, so a
    // consumer that ignores the live lines loses nothing.
    let settled: Vec<&str> = result.lines.iter().map(|line| line.text.as_str()).collect();
    assert_eq!(settled, texts);
    assert_eq!(result.styles.len(), 1);
    assert!(result.stdout.contains("formatted 2 files"));
}

#[test]
fn both_pipes_of_a_running_hook_reach_the_stream() {
    if !posix_shell_available() {
        eprintln!("skipping: no POSIX shell on this host");
        return;
    }
    let repo = TestRepo::init("hooks-stream-pipes");
    install_script(
        &repo,
        "pre-commit",
        &shell_script("echo first\necho reported 1>&2\necho second\n"),
    );

    let sink = Arc::new(Mutex::new(Vec::new()));
    let received = Arc::clone(&sink);
    let result = run_commit_hook_streamed(&repo.repo, "pre-commit", move |chunk| {
        received.lock().unwrap().push(chunk);
    })
    .unwrap();

    let mut texts: Vec<String> = sink
        .lock()
        .unwrap()
        .iter()
        .map(|chunk| chunk.line.text.clone())
        .collect();
    texts.sort();
    assert_eq!(texts, ["first", "reported", "second"]);
    // Each pipe keeps its own raw text in the result.
    assert_eq!(result.stdout, "first\nsecond\n");
    assert_eq!(result.stderr, "reported\n");
}
