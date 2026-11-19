import { execFileSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(scriptDirectory, "..");
const outputDirectory = resolve(
    process.argv[2] ?? join(projectRoot, ".screenshot-workspace")
);

const DAY_MS = 24 * 60 * 60 * 1000;
const now = Date.now();

function readIdentity() {
    const read = (key, fallback) => {
        try {
            return (
                execFileSync("git", ["-C", projectRoot, "config", key], {
                    encoding: "utf8",
                    stdio: ["ignore", "pipe", "ignore"],
                }).trim() || fallback
            );
        } catch {
            return fallback;
        }
    };
    return {
        name: read("user.name", "gitau"),
        email: read("user.email", "gitau@example.test"),
    };
}

const identity = readIdentity();
const baseEnvironment = {
    ...process.env,
    GIT_AUTHOR_NAME: identity.name,
    GIT_AUTHOR_EMAIL: identity.email,
    GIT_COMMITTER_NAME: identity.name,
    GIT_COMMITTER_EMAIL: identity.email,
};

function timestampFor(daysAgo) {
    return new Date(now - daysAgo * DAY_MS).toISOString();
}

function runGit(repository, args, environment = {}) {
    return execFileSync("git", ["-C", repository, ...args], {
        env: { ...baseEnvironment, ...environment },
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
    }).trim();
}

function write(repository, path, content) {
    const target = join(repository, path);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, content);
}

function init(name, origin, defaultBranch = "main") {
    const repository = join(outputDirectory, name);
    mkdirSync(repository, { recursive: true });
    execFileSync(
        "git",
        ["-c", `init.defaultBranch=${defaultBranch}`, "init", repository],
        { env: baseEnvironment, stdio: "ignore" }
    );
    runGit(repository, ["config", "core.autocrlf", "false"]);
    // The origin only exists so the sidebar resolves the owner avatar the
    // same way it would for a real clone.
    runGit(repository, ["remote", "add", "origin", origin]);
    return repository;
}

function commit(repository, message, daysAgo) {
    const date = timestampFor(daysAgo);
    runGit(repository, ["add", "--all"]);
    runGit(
        repository,
        [
            "-c",
            "commit.gpgSign=false",
            "commit",
            "--allow-empty",
            "-m",
            message,
        ],
        { GIT_AUTHOR_DATE: date, GIT_COMMITTER_DATE: date }
    );
}

function merge(repository, branch, message, daysAgo) {
    const date = timestampFor(daysAgo);
    runGit(repository, ["merge", "--no-ff", branch, "-m", message], {
        GIT_AUTHOR_DATE: date,
        GIT_COMMITTER_DATE: date,
    });
}

function firstChangeId(repository) {
    const raw = execFileSync(
        "git",
        ["-C", repository, "status", "--porcelain=v1", "-z"],
        { encoding: "utf8" }
    );
    const unmerged = new Set(["DD", "AU", "UD", "UA", "DU", "AA", "UU"]);
    const preferred = /\.(rs|ts|tsx|js|jsx|mjs|css|json|toml|md)$/;
    const candidates = [];
    const tokens = raw.split("\0");
    for (let index = 0; index < tokens.length; index += 1) {
        const entry = tokens[index];
        if (!entry) continue;
        const status = entry.slice(0, 2);
        const path = entry.slice(3);
        if (status[0] === "R" || status[0] === "C") index += 1;
        if (status === "??") {
            candidates.push({ id: `worktree:${path}`, path });
            continue;
        }
        if (unmerged.has(status)) {
            candidates.push({ id: `conflict:${path}`, path });
            continue;
        }
        if (status[1] !== " ") {
            candidates.push({ id: `worktree:${path}`, path });
            continue;
        }
        if (status[0] !== " ") candidates.push({ id: `index:${path}`, path });
    }
    const chosen =
        candidates.find((candidate) => preferred.test(candidate.path)) ??
        candidates[0];
    return chosen?.id ?? null;
}

function createTauri() {
    const repository = init("tauri", "https://github.com/tauri-apps/tauri.git");

    write(
        repository,
        "README.md",
        `# tauri

Build smaller, faster, and more secure desktop and mobile applications with a
web frontend.
`
    );
    write(
        repository,
        "Cargo.toml",
        `[workspace]
members = ["crates/tauri", "crates/tauri-runtime", "crates/tauri-utils"]
resolver = "2"
`
    );
    write(
        repository,
        "crates/tauri/Cargo.toml",
        `[package]
name = "tauri"
version = "2.0.0"
edition = "2021"

[dependencies]
tauri-runtime = { path = "../tauri-runtime" }
tauri-utils = { path = "../tauri-utils" }
`
    );
    write(
        repository,
        "crates/tauri/src/app.rs",
        `use crate::window::Window;
use crate::Runtime;

pub struct App<R: Runtime> {
    runtime: R,
    windows: Vec<Window<R>>,
}

impl<R: Runtime> App<R> {
    pub fn new(runtime: R) -> Self {
        Self {
            runtime,
            windows: Vec::new(),
        }
    }

    pub fn create_window(&mut self, label: &str) -> &Window<R> {
        self.windows.push(Window::new(label.to_owned()));
        self.windows.last().expect("just pushed")
    }

    pub fn run(&mut self) {
        self.runtime.run();
    }
}
`
    );
    write(
        repository,
        "crates/tauri/src/window/mod.rs",
        `use crate::Runtime;

pub struct Window<R: Runtime> {
    label: String,
    _runtime: std::marker::PhantomData<R>,
}

impl<R: Runtime> Window<R> {
    pub fn new(label: String) -> Self {
        Self {
            label,
            _runtime: std::marker::PhantomData,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}
`
    );
    write(
        repository,
        "crates/tauri-runtime/src/lib.rs",
        `pub trait Runtime {
    type Handle;

    fn run(&self);

    fn window(&self, label: &str) -> Self::Handle;
}
`
    );
    write(
        repository,
        "crates/tauri-utils/src/config.rs",
        `use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
}

pub fn parse(json: &str) -> Result<WindowConfig, serde_json::Error> {
    serde_json::from_str(json)
}
`
    );
    write(
        repository,
        "docs/architecture.md",
        `# Architecture

- \`crates/tauri\` owns the application and window API
- \`crates/tauri-runtime\` is the runtime boundary
- \`crates/tauri-utils\` parses configuration
`
    );
    write(
        repository,
        "docs/CHANGELOG.md",
        `# Changelog

## Unreleased

- Window effects
`
    );

    commit(repository, "Split the runtime from the app API", 24);
    write(
        repository,
        "crates/tauri-utils/src/config.rs",
        `use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub transparent: bool,
}

pub fn parse(json: &str) -> Result<WindowConfig, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn defaults() -> WindowConfig {
    WindowConfig {
        title: "tauri".to_owned(),
        width: 800,
        height: 600,
        resizable: true,
        transparent: false,
    }
}
`
    );
    commit(repository, "Expand the window config", 19);
    runGit(repository, ["tag", "-a", "v2.0.0", "-m", "v2.0.0"], {
        GIT_COMMITTER_DATE: timestampFor(19),
    });

    runGit(repository, ["checkout", "-b", "feat/window-effects"]);
    write(
        repository,
        "crates/tauri/src/window/mod.rs",
        `use crate::Runtime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Blur,
    Acrylic,
    Mica,
}

pub struct Window<R: Runtime> {
    label: String,
    effects: Vec<Effect>,
    _runtime: std::marker::PhantomData<R>,
}

impl<R: Runtime> Window<R> {
    pub fn new(label: String) -> Self {
        Self {
            label,
            effects: Vec::new(),
            _runtime: std::marker::PhantomData,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn set_effects(&mut self, effects: Vec<Effect>) {
        self.effects = effects;
    }

    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
}
`
    );
    commit(repository, "Add the window effects API", 15);
    write(
        repository,
        "crates/tauri-runtime/src/lib.rs",
        `pub trait Runtime {
    type Handle;

    fn run(&self);

    fn window(&self, label: &str) -> Self::Handle;

    fn apply_effects(&self, label: &str, effects: &[u8]);
}
`
    );
    commit(repository, "Wire effects into the runtime", 14);
    const selectedCommit = runGit(repository, ["rev-parse", "HEAD"]);
    write(
        repository,
        "crates/tauri/tests/effects.rs",
        `use tauri::window::Effect;

#[test]
fn effects_are_ordered() {
    let effects = [Effect::Blur, Effect::Acrylic];
    assert_eq!(effects[0], Effect::Blur);
}
`
    );
    commit(repository, "Cover effect parsing", 13);

    runGit(repository, ["checkout", "main"]);
    runGit(repository, ["checkout", "-b", "fix/updater-signature"]);
    write(
        repository,
        "crates/tauri/src/updater.rs",
        `pub struct Signature<'a>(pub &'a str);

pub fn verify(signature: Signature<'_>, payload: &[u8]) -> bool {
    signature.0.len() == 64 && !payload.is_empty()
}
`
    );
    commit(repository, "Verify updater signatures before install", 9);
    write(
        repository,
        "crates/tauri/src/updater.rs",
        `pub struct Signature<'a>(pub &'a str);

pub fn verify(signature: Signature<'_>, payload: &[u8]) -> bool {
    signature.0.len() == 64 && !payload.is_empty()
}

pub fn verify_with_retry(signature: Signature<'_>, payload: &[u8]) -> bool {
    verify(signature, payload) || verify(signature, payload)
}
`
    );
    commit(repository, "Retry signature verification once", 8);

    runGit(repository, ["checkout", "main"]);
    merge(
        repository,
        "feat/window-effects",
        "Merge branch 'feat/window-effects'",
        6
    );
    merge(
        repository,
        "fix/updater-signature",
        "Merge branch 'fix/updater-signature'",
        4
    );
    write(
        repository,
        "crates/tauri/Cargo.toml",
        `[package]
name = "tauri"
version = "2.1.0"
edition = "2021"

[dependencies]
tauri-runtime = { path = "../tauri-runtime" }
tauri-utils = { path = "../tauri-utils" }
`
    );
    commit(repository, "Release v2.1.0", 3);
    runGit(repository, ["tag", "-a", "v2.1.0", "-m", "v2.1.0"], {
        GIT_COMMITTER_DATE: timestampFor(3),
    });

    write(
        repository,
        "crates/tauri/src/app.rs",
        `use crate::window::Window;
use crate::Runtime;

pub struct App<R: Runtime> {
    runtime: R,
    windows: Vec<Window<R>>,
}

impl<R: Runtime> App<R> {
    pub fn new(runtime: R) -> Self {
        Self {
            runtime,
            windows: Vec::new(),
        }
    }

    pub fn create_window(&mut self, label: &str) -> &Window<R> {
        self.windows.push(Window::new(label.to_owned()));
        self.windows.last().expect("just pushed")
    }

    pub fn window(&self, label: &str) -> Option<&Window<R>> {
        self.windows
            .iter()
            .find(|window| window.label() == label)
    }

    pub fn window_labels(&self) -> Vec<&str> {
        self.windows.iter().map(Window::label).collect()
    }

    pub fn run(&mut self) {
        self.runtime.run();
    }
}
`
    );
    write(
        repository,
        "crates/tauri-utils/src/config.rs",
        `use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
    pub transparent: bool,
    pub visible: bool,
}

pub fn parse(json: &str) -> Result<WindowConfig, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn defaults() -> WindowConfig {
    WindowConfig {
        title: "tauri".to_owned(),
        width: 800,
        height: 600,
        resizable: true,
        transparent: false,
        visible: true,
    }
}
`
    );
    write(
        repository,
        "crates/tauri/src/window/mod.rs",
        `pub mod effects;

use crate::Runtime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Blur,
    Acrylic,
    Mica,
}

pub struct Window<R: Runtime> {
    label: String,
    effects: Vec<Effect>,
    _runtime: std::marker::PhantomData<R>,
}

impl<R: Runtime> Window<R> {
    pub fn new(label: String) -> Self {
        Self {
            label,
            effects: Vec::new(),
            _runtime: std::marker::PhantomData,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn set_effects(&mut self, effects: Vec<Effect>) {
        self.effects = effects;
    }

    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
}
`
    );
    write(
        repository,
        "crates/tauri/src/window/effects.rs",
        `use super::Effect;

pub fn encode(effects: &[Effect]) -> Vec<u8> {
    effects
        .iter()
        .map(|effect| match effect {
            Effect::Blur => 1,
            Effect::Acrylic => 2,
            Effect::Mica => 3,
        })
        .collect()
}
`
    );
    write(
        repository,
        "crates/tauri/tests/effects.rs",
        `use tauri::window::{effects, Effect};

#[test]
fn effects_are_ordered() {
    let effects = [Effect::Blur, Effect::Acrylic];
    assert_eq!(effects[0], Effect::Blur);
}

#[test]
fn effects_encode_to_runtime_codes() {
    let encoded = effects::encode(&[Effect::Mica, Effect::Blur]);
    assert_eq!(encoded, vec![3, 1]);
}
`
    );
    runGit(repository, ["add", "crates/tauri/tests/effects.rs"]);
    rmSync(join(repository, "docs/architecture.md"));
    write(
        repository,
        "docs/CHANGELOG.md",
        `# Changelog

## Unreleased

- Window effects
- Visible window config

## 2.1.0

- Updater signature verification
`
    );
    runGit(
        repository,
        ["stash", "push", "-m", "wip: changelog", "--", "docs/CHANGELOG.md"],
        {
            GIT_AUTHOR_DATE: timestampFor(1),
            GIT_COMMITTER_DATE: timestampFor(1),
        }
    );

    return { repository, selectedCommit };
}

function createGit() {
    const repository = init("git", "https://github.com/git/git.git", "master");

    write(
        repository,
        "README.md",
        `# Git

Git is a fast, scalable, distributed revision control system.
`
    );
    write(
        repository,
        "Makefile",
        `CC = cc
CFLAGS = -O2 -Wall

PROGRAMS = git

all: $(PROGRAMS)

git: builtin/status.o diff.o wt-status.o
	$(CC) -o $@ $^
`
    );
    write(
        repository,
        "builtin/status.c",
        `#include "builtin.h"
#include "config.h"
#include "wt-status.h"

int cmd_status(int argc, const char **argv, const char *prefix)
{
	struct wt_status s;
	int short_format = 0;

	git_config(git_status_config, NULL);
	wt_status_prepare(&s);
	s.short_format = short_format;
	return wt_status_print(&s);
}
`
    );
    write(
        repository,
        "wt-status.c",
        `#include "wt-status.h"
#include "color.h"

void wt_status_prepare(struct wt_status *s)
{
	memset(s, 0, sizeof(*s));
	s->show_untracked_files = SHOW_NORMAL_UNTRACKED_FILES;
}

int wt_status_print(struct wt_status *s)
{
	if (s->short_format)
		return wt_status_print_short(s);
	return wt_status_print_long(s);
}
`
    );
    write(
        repository,
        "diff.c",
        `#include "diff.h"

static int detect_rename = 0;

void diff_setup(struct diff_options *o)
{
	memset(o, 0, sizeof(*o));
	o->detect_rename = detect_rename;
}
`
    );
    write(
        repository,
        "Documentation/git-status.txt",
        `git-status(1)
=============

NAME
----
git-status - Show the working tree status
`
    );
    write(
        repository,
        "t/t7508-status.sh",
        `#!/bin/sh

test_description='git status'

. ./test-lib.sh

test_expect_success 'status shows modified files' '
	echo changed >file &&
	git add file &&
	git commit -m initial &&
	echo more >>file &&
	git status --short >actual &&
	grep " M file" actual
'

test_done
`
    );

    commit(
        repository,
        "status: move short format rendering into wt-status",
        28
    );
    runGit(repository, ["tag", "-a", "v2.45.0", "-m", "v2.45.0"], {
        GIT_COMMITTER_DATE: timestampFor(28),
    });

    runGit(repository, ["checkout", "-b", "next"]);
    write(
        repository,
        "wt-status.c",
        `#include "wt-status.h"
#include "color.h"

void wt_status_prepare(struct wt_status *s)
{
	memset(s, 0, sizeof(*s));
	s->show_untracked_files = SHOW_NORMAL_UNTRACKED_FILES;
	s->column = COLUMN_AUTO;
}

int wt_status_print(struct wt_status *s)
{
	if (s->column)
		return wt_status_print_columns(s);
	if (s->short_format)
		return wt_status_print_short(s);
	return wt_status_print_long(s);
}
`
    );
    commit(repository, "status: add --column=auto", 21);
    write(
        repository,
        "diff.c",
        `#include "diff.h"

static int detect_rename = 0;
static int rename_limit = 1000;

void diff_setup(struct diff_options *o)
{
	memset(o, 0, sizeof(*o));
	o->detect_rename = detect_rename;
	o->rename_limit = rename_limit;
}
`
    );
    commit(repository, "diff: honor rename limits in status", 18);
    const selectedCommit = runGit(repository, ["rev-parse", "HEAD"]);
    write(
        repository,
        "t/t7508-status.sh",
        `#!/bin/sh

test_description='git status'

. ./test-lib.sh

test_expect_success 'status shows modified files' '
	echo changed >file &&
	git add file &&
	git commit -m initial &&
	echo more >>file &&
	git status --short >actual &&
	grep " M file" actual
'

test_expect_success 'status --column lays out entries' '
	git status --column=always >actual &&
	grep " M file" actual
'

test_done
`
    );
    commit(repository, "t7508: cover column output", 16);

    runGit(repository, ["checkout", "master"]);
    runGit(repository, ["checkout", "-b", "maint"]);
    write(
        repository,
        "builtin/status.c",
        `#include "builtin.h"
#include "config.h"
#include "wt-status.h"

int cmd_status(int argc, const char **argv, const char *prefix)
{
	struct wt_status s;
	int short_format = 0;
	int untracked_cache = 1;

	git_config(git_status_config, NULL);
	wt_status_prepare(&s);
	s.short_format = short_format;
	s.untracked_cache = untracked_cache;
	return wt_status_print(&s);
}
`
    );
    commit(repository, "status: refresh index before untracked scan", 14);
    write(
        repository,
        "builtin/status.c",
        `#include "builtin.h"
#include "config.h"
#include "wt-status.h"

int cmd_status(int argc, const char **argv, const char *prefix)
{
	struct wt_status s;
	int short_format = 0;
	int untracked_cache = 1;

	git_config(git_status_config, NULL);
	wt_status_prepare(&s);
	s.short_format = short_format;
	s.untracked_cache = untracked_cache;
	refresh_index(&the_index, REFRESH_QUIET, NULL, NULL, NULL);
	return wt_status_print(&s);
}
`
    );
    commit(
        repository,
        "status: fix untracked cache on case-folding filesystems",
        12
    );

    runGit(repository, ["checkout", "master"]);
    merge(repository, "next", "Merge branch 'next'", 10);
    merge(repository, "maint", "Merge branch 'maint'", 8);
    write(
        repository,
        "Documentation/git-status.txt",
        `git-status(1)
=============

NAME
----
git-status - Show the working tree status

OPTIONS
-------
--column[=<style>]::
	List untracked files in columns.
`
    );
    commit(repository, "Documentation: describe status --column", 6);
    runGit(repository, ["tag", "-a", "v2.46.0", "-m", "v2.46.0"], {
        GIT_COMMITTER_DATE: timestampFor(6),
    });

    return { repository, selectedCommit };
}

rmSync(outputDirectory, { recursive: true, force: true });
mkdirSync(outputDirectory, { recursive: true });

const tauri = createTauri();
const git = createGit();

const gitauChange = firstChangeId(projectRoot);

const session = {
    version: 1,
    screenshot: true,
    activeTabId: "t1",
    tabs: [
        {
            tabId: "t1",
            title: "gitau",
            repoPath: projectRoot,
            lastResolvedHref: gitauChange
                ? `/repo/1?change=${encodeURIComponent(gitauChange)}`
                : "/repo/1",
            titleBadge: "repo",
        },
        {
            tabId: "t2",
            title: "tauri",
            repoPath: resolve(tauri.repository),
            lastResolvedHref: `/repo/2?change=${encodeURIComponent(
                "worktree:crates/tauri/src/app.rs"
            )}`,
            titleBadge: "repo",
        },
        {
            tabId: "t3",
            title: "git",
            repoPath: resolve(git.repository),
            lastResolvedHref: `/repo/3?tab=history&view=graph&commit=${git.selectedCommit}`,
            titleBadge: "repo",
        },
    ],
    repositories: [projectRoot, tauri.repository, git.repository].map(
        (path, index) => ({
            path: resolve(path),
            addedAt: index + 1,
        })
    ),
};

const sessionFile = join(outputDirectory, "session.json");
writeFileSync(sessionFile, `${JSON.stringify(session, null, 4)}\n`);

console.log(`Created the fixture repositories in ${outputDirectory}`);
console.log(`Session file: ${sessionFile}`);
console.log(`Run: just screenshots`);
