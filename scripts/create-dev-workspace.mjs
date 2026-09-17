import { execFileSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const projectRoot = resolve(scriptDirectory, "..");
const outputDirectory = resolve(
    process.argv[2] ?? join(projectRoot, ".dev-workspace")
);
const gitEnvironment = {
    ...process.env,
    GIT_AUTHOR_NAME: "Dev Workspace",
    GIT_AUTHOR_EMAIL: "dev-workspace@example.test",
    GIT_COMMITTER_NAME: "Dev Workspace",
    GIT_COMMITTER_EMAIL: "dev-workspace@example.test",
};

function runGit(repository, ...args) {
    return execFileSync("git", ["-C", repository, ...args], {
        env: gitEnvironment,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "pipe"],
    }).trim();
}

function write(repository, path, content) {
    const target = join(repository, path);
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, content);
}

function init(name, options = {}) {
    const repository = join(outputDirectory, name);
    mkdirSync(repository, { recursive: true });
    // init.defaultBranch must be set on the init command itself; configuring
    // it afterwards leaves the already-created initial branch behind.
    execFileSync(
        "git",
        [
            "-c",
            "init.defaultBranch=main",
            "init",
            ...(options.bare ? ["--bare"] : []),
            repository,
        ],
        {
            env: gitEnvironment,
            stdio: "ignore",
        }
    );
    if (!options.bare) {
        runGit(repository, "config", "core.autocrlf", "false");
    }
    return repository;
}

function commit(repository, message, date = "2024-01-01T12:00:00Z") {
    runGit(repository, "add", "--all");
    runGit(
        repository,
        "-c",
        `commit.gpgSign=false`,
        "commit",
        "--allow-empty",
        "-m",
        message
    );
    const commitDate = new Date(date).toISOString();
    runGit(repository, "commit", "--amend", "--no-edit", "--date", commitDate);
}

function makeRepositories() {
    const repositories = [];

    const empty = init("01-empty-unborn");
    repositories.push(empty);

    const detached = init("02-detached-head");
    write(detached, "README.md", "A repository with a detached HEAD.\n");
    commit(detached, "root commit");
    const detachedCommit = runGit(detached, "rev-parse", "HEAD");
    runGit(detached, "checkout", "--detach", detachedCommit);
    repositories.push(detached);

    const dirty = init("03-dirty-conflicted");
    write(dirty, "tracked.txt", "base\n");
    write(dirty, "rename-me.txt", "rename me\n");
    commit(dirty, "base files");
    write(dirty, "tracked.txt", "unstaged edit\n");
    runGit(dirty, "mv", "rename-me.txt", "renamed.txt");
    write(dirty, "untracked dir/odd name.txt", "untracked\n");
    repositories.push(dirty);

    const conflict = init("04-merge-conflict");
    write(conflict, "conflict.txt", "main\n");
    commit(conflict, "common ancestor");
    runGit(conflict, "checkout", "-b", "side-branch");
    write(conflict, "conflict.txt", "side\n");
    commit(conflict, "side change");
    // main must branch from the common ancestor, not from the side tip,
    // or merging side-branch back is already up to date and no conflict
    // is produced.
    runGit(conflict, "checkout", "main");
    write(conflict, "conflict.txt", "main change\n");
    commit(conflict, "main change");
    try {
        runGit(conflict, "merge", "side-branch");
    } catch {
        // The unresolved index is the fixture.
    }
    if (!runGit(conflict, "ls-files", "-u")) {
        throw new Error("04-merge-conflict has no unmerged paths");
    }
    repositories.push(conflict);

    const unusual = init("05-unusual-names");
    write(unusual, "space name.md", "spaces\n");
    write(unusual, "unicode/naïve café.txt", "unicode\n");
    write(unusual, "nested/.hidden", "hidden\n");
    commit(unusual, "files with unusual names");
    runGit(unusual, "tag", "release-with-spaces");
    runGit(unusual, "checkout", "-b", "feature/with-spaces");
    repositories.push(unusual);

    const manyCommits = init("06-many-commits");
    for (let index = 1; index <= 12; index += 1) {
        write(manyCommits, "history.txt", `commit ${index}\n`);
        commit(
            manyCommits,
            `history commit ${index}`,
            `2024-01-${String(index).padStart(2, "0")}T12:00:00Z`
        );
    }
    repositories.push(manyCommits);

    const bare = init("07-bare.git", { bare: true });
    repositories.push(bare);

    const linked = init("08-worktree-main");
    write(linked, "README.md", "Main worktree\n");
    commit(linked, "main worktree");
    runGit(
        linked,
        "worktree",
        "add",
        "../08-worktree-linked",
        "-b",
        "linked-branch"
    );
    repositories.push(linked, join(outputDirectory, "08-worktree-linked"));

    return repositories;
}

rmSync(outputDirectory, { recursive: true, force: true });
mkdirSync(outputDirectory, { recursive: true });
const repositories = makeRepositories();
const now = Date.now();
const session = {
    version: 1,
    tabs: [],
    activeTabId: null,
    repositories: repositories.map((path, index) => ({
        path: resolve(path),
        addedAt: now + index,
    })),
};
writeFileSync(
    join(outputDirectory, "session.json"),
    `${JSON.stringify(session, null, 4)}\n`
);
console.log(
    `Created ${repositories.length} repositories in ${outputDirectory}`
);
const sessionFile = join(outputDirectory, "session.json");
console.log(`Session file: ${sessionFile}`);
console.log(`Run: npm run tauri dev -- -- -- --session "${sessionFile}"`);
