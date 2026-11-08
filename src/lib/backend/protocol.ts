/**
 * GENERATED FILE - do not edit by hand.
 *
 * Source of truth: Rust serde attributes and #[tauri::command] signatures.
 * Regenerate with: cargo run -p contract --bin export_contract
 * Freshness is verified by cargo test (contract::tests::generated_contracts_are_fresh).
 */

import type { Channel } from "@tauri-apps/api/core";
import type { SessionDocument, SyntaxStyle } from "./protocol.manual";

export * from "./protocol.manual";

/**
 * Public identity of the connected GitHub account. Never carries the
 * access token; the token lives only in the OS keychain.
 */
export interface AccountProfile {
    login: string;
    name: string | null;
    avatarUrl: string;
    htmlUrl: string;
    /**
     * Scopes granted to the stored token, as reported by GitHub.
     */
    scopes: string[];
    /**
     * Epoch ms when the connection was established.
     */
    connectedAtMs: number;
}

/**
 * One step of a run's job, in the order it ran.
 */
export interface ActionStep {
    number: number;
    name: string;
    status: string;
    conclusion?: string;
    startedAt?: string;
    completedAt?: string;
    /**
     * The log lines this step produced, sliced out of the job log by
     * timestamp, with any ANSI colour escapes removed.
     */
    log: string;
    /**
     * Span triples per line of `log`, indexing `CheckRunLog::styles`.
     */
    spansByLine: number[][];
}

/**
 * Commits `head` has that `base` does not, and the reverse.
 */
export interface AheadBehind {
    ahead: number;
    behind: number;
}

/**
 * User-facing half of an in-progress device flow. The device code stays in
 * the backend; only the code the user must type crosses IPC.
 */
export interface AuthChallenge {
    userCode: string;
    verificationUri: string;
    expiresInSecs: number;
}

export interface BlameLine {
    lineNo: number;
    commit: ObjectId;
    boundary: boolean;
}

export interface BlameResult {
    path: RelativePath;
    revision?: ObjectId;
    lines: BlameLine[];
}

export interface BranchInfo {
    name: RefName;
    target: ObjectId;
    isHead: boolean;
    upstream?: UpstreamRef;
}

/**
 * A resolved icon served from the disk cache, base64-encoded so the UI can
 * use it directly as an `<img src>` without filesystem access.
 */
export interface CachedIcon {
    key: string;
    dataUrl: string;
    contentType: string;
    /**
     * When the cached bytes were fetched (epoch ms).
     */
    fetchedAtMs: number;
}

/**
 * What a provider can do. The UI reads this to show only the views,
 * actions, and nav entries the connected provider supports, so a provider
 * that lacks CI still renders without dead controls.
 */
export interface Capabilities {
    /**
     * Read issue lists and detail.
     */
    issues: boolean;
    /**
     * Read pull request lists, detail, reviews, and commits.
     */
    pullRequests: boolean;
    /**
     * Submit reviews and review comments.
     */
    reviews: boolean;
    /**
     * Read the notification inbox.
     */
    notifications: boolean;
    /**
     * Read CI check runs and their logs.
     */
    checks: boolean;
    /**
     * Read legacy commit statuses alongside check runs.
     */
    commitStatuses: boolean;
    /**
     * Read workflow runs for a commit.
     */
    workflowRuns: boolean;
    /**
     * Create and update issues, comments, labels, and assignees.
     */
    issuesWrite: boolean;
    /**
     * Update and merge pull requests.
     */
    pullRequestsWrite: boolean;
    /**
     * Create a repository on the provider and push to it.
     */
    publish: boolean;
    /**
     * List the signed-in user's organizations for the publish owner picker.
     */
    orgs: boolean;
}

export type ChangeKind =
    "added"
    | "deleted"
    | "modified"
    | "typeChanged"
    | "renamed"
    | "copied"
    | "conflicted"
    | "untracked";

export type ChangeSide =
    "index"
    | "worktree";

export interface ChartBucket {
    /**
     * UTC epoch seconds of the bucket's inclusive start.
     */
    startSeconds: number;
    /**
     * UTC epoch seconds of the bucket's exclusive end.
     */
    endSeconds: number;
    commits: number;
    additions: number;
    deletions: number;
}

/**
 * Time window covered by one chart bucket.
 */
export type ChartBucketSize =
    "hour"
    | "quarterDay"
    | "halfDay"
    | "day"
    | "twoDays"
    | "halfWeek"
    | "week"
    | "month"
    | "quarter";

/**
 * One annotation on a check run. Test runners report failures here, so this
 * is where the reason a run failed actually lives.
 */
export interface CheckAnnotation {
    path: string;
    startLine?: number;
    endLine?: number;
    startColumn?: number;
    annotationLevel: string;
    message: string;
    title: string;
}

/**
 * One CI check run on a commit. `status` is GitHub's progress
 * (`queued`, `in_progress`, `completed`); `conclusion` stays absent until
 * the run completes.
 */
export interface CheckRun {
    id: number;
    name: string;
    status: string;
    conclusion?: string;
    detailsUrl?: string;
    startedAt: string;
    completedAt?: string;
}

/**
 * One check run with its output and annotations, for the results dialog.
 */
export type CheckRunDetail = CheckRun & {
    output: CheckRunOutput;
    annotations: CheckAnnotation[];
};

/**
 * A run's job log, split the way GitHub's job view splits it. The check run
 * endpoint does not carry log text: `output.text` is null for Actions jobs
 * and the real log only exists on the job.
 */
export interface CheckRunLog {
    steps: ActionStep[];
    /**
     * The 1-based table every step's spans index into, carrying the log's
     * own colours rather than any syntax theme.
     */
    styles: SyntaxStyle[];
    /**
     * The job log exactly as the endpoint returned it, before any splitting.
     * Kept so the real shape of a log can be read from a live run instead of
     * inferred, since the step stamps and the `##[group]` markers do not line
     * up with the steps the way the splitting assumes.
     */
    raw: string;
    /**
     * Set when the run has no job log to show, with the reason.
     */
    unavailable?: string;
}

/**
 * The output a check run produced. Only the single-run read reports this;
 * the list omits it.
 */
export interface CheckRunOutput {
    title: string;
    summary: string;
    /**
     * The raw log text, which can be very large.
     */
    text: string;
    annotationsCount: number;
}

export type CheckoutMode =
    "safe"
    | "force"
    | "keepChanges"
    | "takeChanges";

/**
 * Terminal states mirror DiffEvent so the frontend can drive one lifecycle
 * model for every streamed operation; `completed` carries the canonical
 * repository path to open.
 */
export type CloneEvent =
    { event: "progress";
operationId: number;
    } & CloneProgress
    | { event: "completed";
operationId: number;
repoPath: string;
    }
    | { event: "failed";
operationId: number;
code: string;
message: string;
    }
    | { event: "cancelled";
operationId: number;
    };

/**
 * Fetch/checkout stages reported while a clone runs.
 */
export type ClonePhase =
    "counting"
    | "receiving"
    | "resolving"
    | "checkingOut";

/**
 * One progress sample from an in-flight clone. `progress` is 0..1 and
 * intentionally 0 where the phase is indeterminate (counting, checkout).
 */
export interface CloneProgress {
    phase: ClonePhase;
    progress: number;
    objectsReceived: number;
    objectsTotal: number | null;
    receivedBytes: number;
}

export interface CloneRequest {
    url?: string;
    destination?: string;
    bare?: boolean;
    depth?: number;
    credential?: CredentialRequest;
}

export interface CommitDetail {
    summary: CommitSummary;
    /**
     * Stats against the first parent; empty for root commits' convenience only.
     */
    files: FileChangeStat[];
    totalAdditions: number;
    totalDeletions: number;
}

/**
 * A finished commit together with every hook that executed around it.
 */
export interface CommitExecution {
    summary: CommitSummary;
    hookRuns: HookRunResult[];
}

/**
 * Progress of a hook the commit pipeline runs. A commit has no per-hook
 * command of its own, so the frontend learns about these from the commit's
 * channel and can show a hook running that nobody started by hand.
 */
export type CommitHookEvent =
    { type: "started";
hook: string;
    }
    | { type: "line";
hook: string;
text: string;
/**
* Style ids index the table these append to.
*/
spans: number[];
styles?: SyntaxStyle[];
    }
    | { type: "settled";
result: HookRunResult;
    };

export interface CommitMessage {
    summaryLine: string;
    message: string;
}

/**
 * One commit status, the older sibling of a check run. Apps that have not
 * moved to the checks API (CodeRabbit among them) report here instead, and
 * GitHub shows both in one place, so the pull request page has to read both.
 * The endpoint already returns only the newest status per context.
 */
export interface CommitStatus {
    id: number;
    /**
     * The reporter's name for the check, e.g. `CodeRabbit`.
     */
    context: string;
    /**
     * `pending`, `success`, `failure`, or `error`.
     */
    state: string;
    /**
     * Free text the reporter attaches, e.g. `Review in progress`.
     */
    description: string;
}

export interface CommitSummary {
    id: ObjectId;
    treeId: ObjectId;
    parentIds: ObjectId[];
    summaryLine: string;
    message: string;
    author: Signature;
    committer: Signature;
    /**
     * Files touched vs the first parent (root commits diff the empty tree).
     */
    filesChanged: number;
    /**
     * Insertions vs the first parent.
     */
    additions: number;
    /**
     * Deletions vs the first parent.
     */
    deletions: number;
    /**
     * Tag names whose target resolves to this commit.
     */
    tags: string[];
    /**
     * Fuzzy-match ranges for the current search, in char offsets of the
     * displayed `summary_line` / `author.name`. Absent when unsearched.
     */
    matchRanges?: SearchMatchRange[];
}

export interface ConflictEntry {
    path: RelativePath;
    sides: ConflictSide[];
}

export interface ConflictFile {
    path: RelativePath;
    stage: number;
    revision: ObjectId;
    binary: boolean;
    rows: DiffRow[];
    styles: ConflictStyle[];
    truncated: boolean;
}

export interface ConflictSide {
    stage: number;
    id: ObjectId;
    mode: number;
}

export interface ConflictStyle {
    light: string;
    dark: string;
    b?: boolean;
    i?: boolean;
    u?: boolean;
}

/**
 * Non-secret profile of a connected account, tagged with its connection.
 */
export interface ConnectionProfile {
    provider: string;
    host: string;
    account: AccountProfile;
}

export interface CreatePullRequestBody {
    title?: string;
    head?: string;
    base?: string;
    body?: string;
    draft?: boolean;
    labels?: string[];
}

/**
 * Request body for creating a repository. The wire form is snake_case, which
 * the provider serializes directly.
 */
export interface CreateRepositoryBody {
    name: string;
    description?: string;
    private: boolean;
}

export interface CreateRepositoryRequest {
    /**
     * Existing directory the repository folder lives in, or the directory a
     * new folder is created inside when `name` is set.
     */
    parentDirectory: string;
    /**
     * Folder to create inside `parent_directory`; empty initializes
     * `parent_directory` itself.
     */
    name: string;
    readme: boolean;
    gitignoreTemplate: string | null;
    license: string | null;
}

/**
 * A freshly created repository plus the canonical path callers should use
 * for opening it later (input parent/name may differ in separators).
 */
export type CreateRepositoryResult = OpenedRepository & {
    path: string;
};

/**
 * A repository the provider created, projected to what publish needs.
 */
export interface CreatedRepository {
    full_name: string;
    html_url: string;
    default_branch: string | null;
    /**
     * HTTPS clone URL as reported by the provider; publish wires this up as
     * `origin`. Tests substitute a local path here.
     */
    clone_url: string | null;
}

export type CredentialKind =
    "default"
    | { usernamePassword: {
    username: string;
    password: string;
} }
    | { sshKey: {
    username: string;
    keyPath: string;
    passphrase: string | null;
} }
    | { inMemorySshKey: {
    username: string;
    keyPem: string;
    passphrase: string | null;
} };

export interface CredentialRequest {
    kind: CredentialKind;
}

export type DiffComparison =
    "workingTree"
    | "unstaged"
    | "staged"
    | { treeToTree: {
    old: RevisionSpec;
    new: RevisionSpec;
} }
    | { commitToParent: {
    commit: RevisionSpec;
} };

/**
 * A window of hidden context lines revealed between diff hunks.
 */
export interface DiffContextResult {
    rows: DiffRow[];
    /**
     * Style entries the rows' span ids index into, relative to this delta.
     */
    styles: SyntaxStyle[];
    moreBelow: boolean;
}

/**
 * Streaming protocol events. Serialized with a `event` tag for TS discriminated unions.
 */
export type DiffEvent =
    { event: "started";
operationId: OperationId;
snapshotId: SnapshotId;
generation: number;
sections: SectionMeta[];
estimatedTotalRows: number;
/**
* Present for commit comparisons so the caller can label the diff
* without a second read.
*/
commitMessage?: CommitMessage;
    }
    | { event: "sectionLayout";
operationId: OperationId;
sectionId: number;
startRow: number;
rowCount: number;
oldLineCount: number;
newLineCount: number;
    }
    | { event: "chunk";
operationId: OperationId;
sectionId: number;
rowStart: number;
/**
* Shared with the operation's stored rows; serializes as a plain
* array on the wire.
*/
rows: SharedRows;
/**
* One entry per row in `rows`; `None` for plain-text rows. Omitted
* entirely when the section is not syntax-highlighted.
*/
spansByRow?: (number[] | null)[];
/**
* Style-table delta for this chunk; spans' third element indexes
* into the per-section table these append to.
*/
styles?: SyntaxStyle[];
    }
    | { event: "layoutReady";
operationId: OperationId;
totalRows: number;
    }
    | { event: "completed";
operationId: OperationId;
totalRows: number;
additions: number;
deletions: number;
durationMs: number;
    }
    | { event: "failed";
operationId: OperationId;
code: string;
message: string;
    }
    | { event: "cancelled";
operationId: OperationId;
    };

export interface DiffImage {
    old?: DiffImageSide;
    new?: DiffImageSide;
}

export interface DiffImageSide {
    /**
     * `data:<mime>;base64,...`; a byte array would cross IPC as a JSON
     * number array, several times larger and parsed element by element.
     */
    dataUrl: string;
}

export interface DiffRequest {
    comparison?: DiffComparison;
    /**
     * Context lines around hunks.
     */
    contextLines?: number;
    interhunkLines?: number;
    detectRenames?: boolean;
    detectCopies?: boolean;
    /**
     * Restrict to these repository-relative paths.
     */
    paths?: string[];
    ignoreWhitespace?: boolean;
}

export interface DiffRow {
    kind: DiffRowKind;
    oldLineno?: number;
    newLineno?: number;
    content: string;
    /**
     * Hex-encoded raw bytes used when the line is not valid UTF-8.
     */
    rawHex?: string;
    /**
     * Flat `[start, len, styleId]` triples covering the non-plain segments
     * of `content`; produced by backend syntax highlighting.
     */
    spans?: number[];
}

export type DiffRowKind =
    "fileHeader"
    | "hunkHeader"
    | "context"
    | "addition"
    | "deletion"
    | "binaryNotice";

export interface FileChangeStat {
    path: RelativePath;
    oldPath?: RelativePath;
    kind: ChangeKind;
    additions: number;
    deletions: number;
    binary: boolean;
    oldMode: number | null;
    newMode: number | null;
    oldId: ObjectId | null;
    newId: ObjectId | null;
}

export interface FileContent {
    path: RelativePath;
    revision: ObjectId;
    data: number[];
    binary: boolean;
    size: number;
}

/**
 * Monotonic counter bumped by every mutation or external change.
 */
export type Generation = number;

/**
 * A commit-lifecycle hook script discovered in the repository hooks
 * directory (`core.hooksPath` aware). Missing standard hooks are not
 * listed; only files that exist on disk appear.
 */
export interface GitHook {
    /**
     * Git hook name without extension, e.g. `pre-commit`.
     */
    name: string;
    /**
     * Absolute path of the script that would execute.
     */
    path: string;
    executable: boolean;
}

export interface GitignoreTemplateInfo {
    id: string;
    label: string;
}

/**
 * One lane-to-lane edge segment drawn across a row's vertical band. The
 * edge starts at `from_lane` on this row and lands on `to_lane` on the next
 * row; straight continuations have equal values and are emitted for every
 * pending lane so through-lines span bands whose commit sits elsewhere.
 * Renderers color bends by `to_lane` so a curve matches the lane it joins.
 */
export interface GraphEdge {
    fromLane: number;
    toLane: number;
}

/**
 * Stream shape for commit-graph operations. Rows arrive in walk order in
 * fixed-size chunks; absolute row positions are stable.
 */
export type GraphEvent =
    { event: "started";
operationId: OperationId;
snapshotId: SnapshotId;
generation: Generation;
    }
    | { event: "chunk";
operationId: OperationId;
rowStart: number;
rows: GraphRow[];
    }
    | { event: "completed";
operationId: OperationId;
totalRows: number;
    }
    | { event: "failed";
operationId: OperationId;
code: string;
message: string;
    }
    | { event: "cancelled";
operationId: OperationId;
    };

/**
 * Tip and exclusions for a commit-graph stream. Mirrors the paging-free
 * subset of HistoryPageQuery: the graph always walks the whole history.
 */
export interface GraphQuery {
    /**
     * Tip of the walk (defaults to HEAD).
     */
    revision?: RevisionSpec;
    /**
     * Hide commits reachable from these revisions.
     */
    excludeReachableFrom?: RevisionSpec[];
}

export interface GraphRangeResult {
    rows: GraphRow[];
    nextCursor: number;
    hasMore: boolean;
    /**
     * Rows materialized so far.
     */
    knownTotalRows: number;
    complete: boolean;
}

export interface GraphRow {
    /**
     * Absolute position in the walk; stable for the operation's lifetime.
     */
    index: number;
    id: ObjectId;
    /**
     * Lane the commit's node occupies.
     */
    lane: number;
    edges: GraphEdge[];
    kind: GraphRowKind;
    summaryLine: string;
    authorName: string;
    authorEmail: string;
    /**
     * Committer time, epoch seconds (matches CommitSummary timestamps).
     */
    timeSeconds: number;
    /**
     * Tag names whose target resolves to this commit.
     */
    tags: string[];
    /**
     * Branch decorations (local and remote-tracking) pointing at this commit.
     */
    refs: string[];
}

export type GraphRowKind =
    "commit"
    | "merge"
    | "root";

export type HeadState =
    { state: "attached";
branch: string;
target: ObjectId;
    }
    | { state: "detached";
target: ObjectId;
    }
    | { state: "unborn";
branch: string;
    };

/**
 * One highlighted markdown code fence. `spans_by_line` aligns with
 * `text.split('\n')` (a trailing empty line simply has no entry);
 * `styles` is the 1-based table the span triples index into, shaped
 * exactly like the diff `SyntaxStyle` the UI already renders.
 */
export interface HighlightedSnippet {
    highlighted: boolean;
    spansByLine: number[][];
    styles: SyntaxStyle[];
}

export interface HistoryChart {
    snapshotId: SnapshotId;
    generation: Generation;
    /**
     * The resolved unit after applying auto-selection.
     */
    bucketSize: ChartBucketSize;
    /**
     * Chronological; gaps between first and last activity are zero-filled
     * so the chart shows quiet periods instead of compressing them away.
     */
    buckets: ChartBucket[];
    /**
     * Commits included in the aggregation (merges excluded by default).
     */
    totalCommits: number;
}

/**
 * Aggregated history for the chart view. Every field is optional so callers
 * can finetune a single knob without repeating the defaults.
 */
export interface HistoryChartQuery {
    /**
     * Tip of the walk (defaults to HEAD).
     */
    revision?: RevisionSpec;
    /**
     * Hide commits reachable from these revisions.
     */
    excludeReachableFrom?: RevisionSpec[];
    /**
     * Fixed bucket unit; `None` auto-selects purely from the time since the
     * first commit: 10y+ monthly, 6y+ weekly, 3y+ half-weekly, 1y+ two-day,
     * 180d+ daily, 90d+ half-daily, 14d+ quarter-daily, younger hourly.
     */
    bucket?: ChartBucketSize;
    /**
     * Merge commits contribute nothing when false (default): no commit
     * count and no diff stats, avoiding double-counted branch lines.
     */
    includeMerges?: boolean;
}

export interface HistoryPage {
    snapshotId: SnapshotId;
    generation: Generation;
    commits: CommitSummary[];
    /**
     * Commits in the walk, independent of the page's `skip`/`limit`.
     */
    total: number;
    hasMore: boolean;
    unpushed: ObjectId[];
}

export interface HistoryPageQuery {
    /**
     * Tip of the walk.
     */
    revision?: RevisionSpec;
    limit?: number;
    skip?: number;
    /**
     * Hide commits reachable from these revisions.
     */
    excludeReachableFrom?: RevisionSpec[];
    /**
     * Only commits touching this path.
     */
    path?: string;
    /**
     * Case-insensitive filter against commit message and author: each
     * whitespace-separated term must substring-match a field or
     * fuzzy-match inside a single word.
     */
    search?: string;
}

/**
 * A commit hook script as editable text. Hooks missing on disk come back
 * `exists: false` with empty content, so the editor can create them.
 */
export interface HookContent {
    /**
     * Git hook name without extension, e.g. `pre-commit`.
     */
    hook: string;
    /**
     * Absolute path the script would be written to.
     */
    path: string;
    /**
     * Whether a script already exists on disk.
     */
    exists: boolean;
    content: string;
}

/**
 * One line of a running hook, delivered as the process writes it. Carries
 * the styles it interned so a consumer can keep one growing table instead of
 * the whole run's.
 */
export type HookOutputChunk = HookOutputLine & {
    /**
     * Style-table delta; the ids in `line.spans` index the table these
     * append to.
     */
    styles?: SyntaxStyle[];
};

/**
 * One line of hook output with the ANSI escapes taken out and the colours
 * resolved into `[start, len, styleId]` triples, the span format the
 * frontend renders for diffs and CI logs.
 */
export interface HookOutputLine {
    text: string;
    /**
     * Style ids index [`HookRunResult::styles`].
     */
    spans: number[];
}

/**
 * Outcome of one hook run. A non-zero exit is still an Ok result; the
 * frontend decides how to present failures.
 */
export interface HookRunResult {
    hook: string;
    /**
     * Process exit code; `None` when terminated by a signal or the
     * executable could not be spawned.
     */
    exitCode: number | null;
    success: boolean;
    stdout: string;
    stderr: string;
    durationMs: number;
    /**
     * Captured output, parsed the way CI logs are, so the frontend renders
     * the hook's own colours instead of raw escapes.
     */
    lines: HookOutputLine[];
    /**
     * Style table `lines[].spans` index.
     */
    styles: SyntaxStyle[];
}

/**
 * One comment on the issue timeline.
 */
export interface IssueComment {
    id: number;
    author: User;
    body: string;
    createdAt: string;
    htmlUrl: string;
}

/**
 * Full issue detail for the issue page header and sidebar.
 */
export interface IssueDetail {
    number: number;
    title: string;
    state: string;
    stateReason: ThreadStateReason;
    body: string;
    author: User;
    labels: Label[];
    assignees: User[];
    participants: User[];
    createdAt: string;
    updatedAt: string;
    htmlUrl: string;
}

/**
 * One non-comment timeline entry (labeled, assigned, closed, ...).
 * `kind` is the raw GitHub event name; the UI maps known kinds to icons.
 */
export interface IssueEvent {
    id: number;
    kind: string;
    actor: string;
    actorAvatarUrl: string;
    createdAt: string;
    label?: string;
    labelColor?: string;
    assignee?: string;
}

/**
 * One row of the issues list. Pull requests are filtered out before
 * reaching the UI, so every item here is a plain issue.
 */
export interface IssueListItem {
    number: number;
    title: string;
    state: string;
    labels: Label[];
    commentCount: number;
    assignees: User[];
    author: string;
    updatedAt: string;
}

/**
 * A label on an issue. `color` is the hex string without `#`.
 */
export interface Label {
    name: string;
    color: string;
}

/**
 * One language's share of the worktree's code.
 */
export interface LanguageShare {
    language: string;
    color: string;
    /**
     * Percent of the classified byte total, 1..=100.
     */
    percent: number;
}

/**
 * One tracked path resolved to LFS: the pointer commit stores its sha256
 * oid and byte size; `materialized` is whether the worktree holds the real
 * bytes instead of the pointer text.
 */
export interface LfsFileInfo {
    path: string;
    oid: string;
    size: number;
    /**
     * Worktree holds the real object bytes (already smudged).
     */
    materialized: boolean;
    /**
     * The object file exists in the repository's LFS object store.
     */
    inStore: boolean;
}

export interface LfsStatus {
    total: number;
    materialized: number;
    files: LfsFileInfo[];
}

export interface LicenseTemplateInfo {
    id: string;
    name: string;
    description: string;
}

/**
 * Merge options. An empty `commit_title` or `commit_message` lets GitHub
 * pick its own default, which is what an unset field means on the wire.
 */
export interface MergePullRequestBody {
    mergeMethod?: PullRequestMergeMethod;
    commitTitle?: string;
    commitMessage?: string;
}

/**
 * Outcome of a merge attempt. `merged` is false when GitHub accepted the
 * call but declined to merge (a policy block, or the sha no longer matches).
 */
export interface MergePullRequestResult {
    sha: string;
    merged: boolean;
    message: string;
}

/**
 * One GitHub notification thread. The token never crosses IPC; only this
 * non-secret projection reaches the UI.
 */
export interface Notification {
    id: string;
    unread: boolean;
    reason: string;
    subjectTitle: string;
    subjectType: string;
    repoFullName: string;
    /**
     * Best-effort web URL derived from the API subject URL.
     */
    htmlUrl: string | null;
    /**
     * The subject's API URL, for on-click resolution of types without a
     * static web mapping (releases, check suites).
     */
    subjectUrl: string | null;
    updatedAt: string;
}

/**
 * One page of the inbox. `has_more` comes from the response `Link`
 * header, so the UI stops exactly when GitHub has no next page.
 */
export interface NotificationPage {
    notifications: Notification[];
    page: number;
    hasMore: boolean;
}

/**
 * A git object id. Stored as raw bytes so SHA-1 and SHA-256 repositories both work.
 */
export type ObjectId = string;

export interface OpenRepositoryRequest {
    /**
     * Filesystem path pointing inside a repository (searches upward).
     */
    path: string;
}

export interface OpenedRepository {
    id: RepoId;
    snapshot: RepoSnapshot;
}

/**
 * Stable handle for a long-running operation (diff stream, fetch, ...).
 */
export type OperationId = number;

export type OperationKind =
    "none"
    | "merge";

export interface OperationState {
    kind: OperationKind;
    message?: string;
    conflictPaths: string[];
    heads: string[];
}

export interface Org {
    login: string;
    avatarUrl?: string;
}

/**
 * A provider's advertised identity and capabilities, sent to the UI so it
 * can gate views without knowing provider names.
 */
export interface ProviderDescriptor {
    id: string;
    label: string;
    hosts: string[];
    imageHosts: string[];
    capabilities: Capabilities;
}

export interface PublishRepositoryRequest {
    /**
     * Provider to publish to. `None` uses the registry's default provider.
     */
    provider?: string;
    /**
     * `None` publishes under the signed-in user's personal account.
     */
    owner: string | null;
    name: string;
    description?: string;
    private: boolean;
}

export interface PublishResult {
    fullName: string;
    htmlUrl: string;
    defaultBranch?: string;
}

/**
 * One commit on a pull request's head branch. `author` falls back to the
 * git author name when the commit is not linked to a GitHub account, so a
 * row always names someone.
 */
export interface PullRequestCommit {
    sha: string;
    message: string;
    author: User;
    authoredAt: string;
    htmlUrl: string;
}

/**
 * Full pull request detail for the pull request page header and sidebar.
 * Carries the issue projection plus everything only the pulls endpoint
 * reports: draft state, mergeability, the two refs, and diff stats.
 */
export interface PullRequestDetail {
    number: number;
    title: string;
    state: string;
    stateReason: ThreadStateReason;
    body: string;
    author: User;
    labels: Label[];
    /**
     * Reviewers requested on the pull request; GitHub returns them in the
     * same slot as issue assignees.
     */
    assignees: User[];
    createdAt: string;
    updatedAt: string;
    htmlUrl: string;
    draft: boolean;
    /**
     * Merge commit time. Set only once merged, which is also how a merged
     * pull request is told apart from a closed one, since both report
     * `state: "closed"`.
     */
    mergedAt?: string;
    /**
     * GitHub's mergeable verdict: null while the merge is still being
     * computed, so the UI must re-read before offering the merge button.
     */
    mergeable: boolean | null;
    /**
     * Why the merge landed where it did, e.g. `clean`, `dirty`, or
     * `blocked`. Empty until the computation finishes.
     */
    mergeableState: string;
    head: PullRequestRef;
    base: PullRequestRef;
    additions: number;
    deletions: number;
    changedFiles: number;
    commits: number;
}

/**
 * One row of the pull requests list. Carries the issue projection plus
 * `merged_at`, which is the only way to tell a merged pull request from a
 * closed one: search reports both as `state: "closed"`.
 */
export interface PullRequestListItem {
    number: number;
    title: string;
    state: string;
    stateReason: ThreadStateReason;
    labels: Label[];
    commentCount: number;
    assignees: User[];
    author: User;
    updatedAt: string;
    mergedAt?: string;
    htmlUrl: string;
    /**
     * `owner/repo` the pull request lives in. A cross-repo search has to
     * derive it from the item URL, since search reports no repository name.
     */
    repoFullName: string;
}

/**
 * How GitHub should combine the branch. Anything but `merge` discards the
 * branch's own commit structure, which is why the UI defaults to `merge`.
 */
export type PullRequestMergeMethod =
    "merge"
    | "squash"
    | "rebase";

/**
 * One side of a pull request, as reported by the pulls endpoint. Only the
 * ref and owning repo are projected; the API also reports a full `repo`
 * object per side.
 */
export interface PullRequestRef {
    ref: string;
    sha: string;
    label: string;
}

/**
 * One submitted review. `state` is GitHub's raw verdict: `APPROVED`,
 * `CHANGES_REQUESTED`, `COMMENTED`, or `DISMISSED`.
 */
export interface PullRequestReview {
    id: number;
    author: User;
    state: string;
    body: string;
    submittedAt: string;
    htmlUrl: string;
}

/**
 * One inline comment anchored to a line in the diff.
 */
export interface PullRequestReviewComment {
    id: number;
    author: User;
    body: string;
    /**
     * Repository-relative path the comment is anchored to.
     */
    path: string;
    /**
     * Line in the new file, absent once the comment is outdated.
     */
    line?: number;
    /**
     * The diff hunk the comment was written against.
     */
    diffHunk: string;
    createdAt: string;
    htmlUrl: string;
    /**
     * Set when this comment replies to another review comment.
     */
    inReplyToId?: number;
    /**
     * The review this comment belongs to. A reviewer writes inline comments
     * before submitting, so timestamps alone place them before their own
     * review.
     */
    pullRequestReviewId?: number;
}

/**
 * Result payload describing what a push changed per refspec.
 */
export interface PushOutcome {
    reference: string;
    accepted: boolean;
    reason?: string;
}

export interface RangeResult {
    rows: RangeRow[];
    nextCursor: number;
    hasMore: boolean;
    knownTotalRows: number;
    complete: boolean;
}

export type RangeRow = DiffRow & {
    sectionId: number;
    path?: RelativePath;
    sectionKind: SectionKind;
};

export interface ReadmeContent {
    path: string;
    content: string;
}

/**
 * Validated git reference-ish names (branch, tag, remote).
 */
export type RefName = string;

/**
 * A validated, repository-relative path using `/` separators.
 */
export type RelativePath = string;

export interface RemoteInfo {
    name: string;
    url?: string;
    pushUrl?: string;
}

export interface RemoteRepoInfo {
    description?: string;
    source?: string;
    stars?: number;
    forks?: number;
    languages?: LanguageShare[];
    fetchedAtMs?: number;
}

/**
 * The outcome of removing a repository: the closed backend session id
 * (absent when the repo had no open session) and the canonical repo root
 * the removal and optional trash applied to.
 */
export interface RemoveRepositoryResult {
    removedRepoId: number | null;
    repoRoot: string;
}

/**
 * Stable handle for an opened repository within this process.
 */
export type RepoId = number;

export interface RepoListing {
    branches: BranchInfo[];
    tags: TagInfo[];
}

/**
 * The signed-in user's access level on a repository. The UI gates
 * editing and deleting other people's comments on `push`.
 */
export interface RepoPermissions {
    push: boolean;
    admin: boolean;
}

export interface RepoSnapshot {
    id: SnapshotId;
    generation: Generation;
    workdir?: string;
    gitDir: string;
    head: HeadState;
    shaKind: ShaKind;
}

export interface RepositoryInvalidation {
    repoId: number;
    generation: number;
    reason: string;
}

/**
 * Public repository metadata, read without a token so it works before any
 * account is connected. Fields the provider does not expose stay `None`.
 */
export interface RepositoryMetadata {
    description?: string;
    stars?: number;
    forks?: number;
}

export type ResetKind =
    "soft"
    | "mixed"
    | "hard";

export type ResolveSide =
    "ours"
    | "theirs";

/**
 * A validated revision expression. Engines resolve it against a repository.
 */
export type RevisionSpec = string;

/**
 * One issue from a GitHub search result. Search items carry `html_url`
 * and `repository_url` instead of a nested `repository.full_name`, so
 * the repo name is derived from the URL.
 */
export interface SearchIssueItem {
    number: number;
    title: string;
    state: string;
    stateReason: ThreadStateReason;
    labels: Label[];
    commentCount: number;
    assignees: User[];
    author: string;
    updatedAt: string;
    htmlUrl: string;
    repoFullName: string;
}

/**
 * One page of search results. `has_more` comes from the response `Link`
 * header.
 */
export interface SearchIssuePage {
    items: SearchIssueItem[];
    page: number;
    hasMore: boolean;
}

export type SearchMatchField =
    "summary"
    | "author";

export interface SearchMatchRange {
    field: SearchMatchField;
    start: number;
    length: number;
}

/**
 * One page of pull request search results.
 */
export interface SearchPullRequestPage {
    items: PullRequestListItem[];
    page: number;
    hasMore: boolean;
}

export type SectionKind =
    "added"
    | "deleted"
    | "modified"
    | "renamed"
    | "copied"
    | "typeChanged"
    | "conflicted";

export interface SectionMeta {
    sectionId: number;
    path: RelativePath;
    oldPath?: RelativePath;
    kind: SectionKind;
    binary: boolean;
    image?: boolean;
    /**
     * True while rows are still being computed.
     */
    complete: boolean;
}

export interface SelectOption {
    value: string;
    label: string;
}

export interface SerializedError {
    code: string;
    message: string;
    retryable: boolean;
    detail?: string;
}

export interface SessionRepository {
    path: string;
    repoId?: number;
    error?: string;
}

/**
 * One renderable/validated setting entry. `setting_type` is flattened so
 * the wire shape is `{ key, label, ..., kind, options?/min?/max?/step? }`.
 */
export type SettingDefinition = SettingType & {
    /**
     * Stable camelCase identifier; also the key inside the values file.
     */
    key: string;
    label: string;
    description?: string;
    defaultValue: SettingValue;
    /**
     * Hidden settings are persisted and hook-accessible but never rendered
     * by the settings UI.
     */
    hidden: boolean;
};

/**
 * The concrete type of a setting. Serialized as a flat discriminated
 * union (`{"kind": "select", "options": [...]}`) so the frontend can
 * render controls directly from it.
 */
export type SettingType =
    { kind: "boolean" }
    | { kind: "select";
options: SelectOption[];
    }
    | { kind: "number";
min: number | null;
max: number | null;
step: number | null;
    }
    | { kind: "string" }
    | { kind: "multilineString" }
    | { kind: "shortcut" }
    | { kind: "stringList" }
    | { kind: "font" };

/**
 * A stored setting value. Deserialized untagged from whatever JSON shape
 * matches; validated against the setting's [`SettingType`] before use.
 */
export type SettingValue =
    boolean
    | number
    | string
    | string[];

export interface SettingsSchema {
    tabs: SettingsTab[];
}

export interface SettingsSection {
    id: string;
    title: string;
    settings: SettingDefinition[];
}

export interface SettingsSnapshot {
    schema: SettingsSchema;
    /**
     * Merged values (stored-or-default) for every schema key. Serialized
     * as a plain camelCase object matching the generated TS interface.
     */
    values: SettingsValues;
    /**
     * Keys explicitly present in the persisted file (used by one-time
     * frontend migrations to distinguish "user picked" from "default").
     */
    savedKeys: string[];
}

export interface SettingsTab {
    id: string;
    label: string;
    sections: SettingsSection[];
}

/**
 * Merged setting values (stored-or-default) keyed by camelCase key.
 */
export type SettingsValues = Record<string, SettingValue>;

export type ShaKind =
    "sha1"
    | "sha256";

/**
 * A ranged view into a shared section allocation; serializes exactly like
 * the `[DiffRow]` slice it covers.
 */
export type SharedRows = DiffRow[];

export interface Signature {
    name: string;
    email: string;
    timeSeconds: number;
    timeOffsetMinutes: number;
}

/**
 * Point-in-time token captured when reading repository state.
 */
export type SnapshotId = number;

export type StashAction =
    "pop"
    | "apply"
    | "drop";

export interface StashEntry {
    index: number;
    message: string;
    commit: string;
}

/**
 * One selectable change. A path can appear twice, once staged and once
 * unstaged, and each row carries its own stable [`StatusEntry::id`] so the
 * UI can select it and request the matching diff.
 */
export interface StatusEntry {
    /**
     * Stable selector: `index:<path>` or `worktree:<path>`.
     */
    id: string;
    side: ChangeSide;
    path: RelativePath;
    oldPath?: RelativePath;
    kind: ChangeKind;
}

/**
 * Options controlling how status is computed.
 */
export interface StatusOptions {
    includeIgnored?: boolean;
    includeUntracked?: boolean;
    recurseUntrackedDirs?: boolean;
}

export interface StatusReport {
    snapshotId: SnapshotId;
    generation: Generation;
    entries: StatusEntry[];
    conflicts: ConflictEntry[];
}


export interface SubmoduleInfo {
    name: string;
    /**
     * Repository-relative path of the submodule directory.
     */
    path: string;
    url?: string;
    branch?: string;
    /**
     * Commit the nested checkout points at, when initialized.
     */
    commit?: string;
    /**
     * Nested repository exists on disk (content available to work in).
     */
    initialized: boolean;
    /**
     * The superproject worktree materialized this submodule.
     */
    checkedOut: boolean;
    /**
     * The nested worktree has staged/worktree/untracked changes.
     */
    dirty: boolean;
}

export interface TagInfo {
    name: RefName;
    target: ObjectId;
    /**
     * For annotated tags this is the tag object itself.
     */
    tagObject?: ObjectId;
    message?: string;
    tagger?: Signature;
}


/**
 * Why a thread ended, from GitHub's `state_reason`. A thread that was never
 * closed carries no reason, and so does one GitHub reports as `reopened`,
 * which describes a state rather than an ending. Anything unrecognised is
 * [`ThreadStateReason::None`], so a new value degrades to "closed" instead
 * of failing the read.
 */
export type ThreadStateReason =
    "none"
    | "completed"
    | "notPlanned"
    | "duplicate";

/**
 * Partial update to an issue: every field left `None` is untouched.
 */
export interface UpdateIssueBody {
    state?: string;
    body?: string;
    labels?: string[];
    assignees?: string[];
}

/**
 * Partial update to a pull request: every field left `None` is untouched.
 * The field names are single words, so the camelCase and snake_case wire
 * conventions agree and this serializes as GitHub expects.
 */
export interface UpdatePullRequestBody {
    state?: string;
    body?: string;
    base?: string;
    draft?: boolean;
}

export interface UpstreamRef {
    remote: RefName;
    branch: RefName;
    ahead: number;
    behind: number;
    /**
     * Commit the remote-tracking ref points at. Lets the client walk
     * remote-only history, e.g. to preview what a force push would drop.
     */
    target?: ObjectId;
}

/**
 * A GitHub user as shown on issues: login plus avatar, never the token.
 */
export interface User {
    login: string;
    avatarUrl: string;
}

export interface ValuesSnapshot {
    values: SettingsValues;
    savedKeys: string[];
}

export type WorkflowOutcome =
    { outcome: "alreadyUpToDate" }
    | { outcome: "fastForwarded";
head: string;
    }
    | { outcome: "merged";
commit: string;
    }
    | { outcome: "conflicted";
paths: string[];
    }
    | { outcome: "started";
totalSteps: number;
    }
    | { outcome: "progressed";
remaining: number;
    }
    | { outcome: "finished";
commit: string | null;
    }
    | { outcome: "aborted" };

/**
 * One workflow run for a commit. Runs are started from GitHub or the app's
 * own CI, not from here, so this is a read model.
 */
export interface WorkflowRun {
    id: number;
    name: string;
    event: string;
    status: string;
    conclusion?: string;
    runNumber: number;
    headBranch: string;
    htmlUrl: string;
    createdAt: string;
}

export interface WorktreeInfo {
    /**
     * Worktree name, unique within the repository. For linked trees this is
     * the libgit2 admin-dir key; the primary tree is reported as `main`.
     */
    name: string;
    path: string;
    /**
     * Short branch name checked out in this tree, when attached.
     */
    branch?: string;
    /**
     * True for the tree this repository session is bound to.
     */
    isCurrent: boolean;
    /**
     * True for the synthetic entry describing the repository's own tree,
     * which is not a linked worktree. A linked tree that happens to be the
     * session's tree has `is_current` but not `is_primary`.
     */
    isPrimary: boolean;
    lockedBy?: string;
    /**
     * True when the tree's checkout directory no longer exists on disk.
     */
    isPrunable: boolean;
}

export type CommandName =
    | "fonts_list"
    | "forge_account"
    | "forge_begin_sign_in"
    | "forge_cancel_sign_in"
    | "forge_complete_sign_in"
    | "forge_connections"
    | "forge_create_issue"
    | "forge_create_issue_comment"
    | "forge_create_pull"
    | "forge_delete_issue_comment"
    | "forge_get_check_run"
    | "forge_get_check_run_log"
    | "forge_get_issue"
    | "forge_get_pull"
    | "forge_list_check_runs"
    | "forge_list_commit_statuses"
    | "forge_list_issue_comments"
    | "forge_list_issue_events"
    | "forge_list_issues"
    | "forge_list_labels"
    | "forge_list_notifications"
    | "forge_list_orgs"
    | "forge_list_pull_commits"
    | "forge_list_pull_requests"
    | "forge_list_pull_review_comments"
    | "forge_list_pull_reviews"
    | "forge_list_workflow_runs"
    | "forge_mark_all_notifications_read"
    | "forge_mark_notification_read"
    | "forge_merge_pull"
    | "forge_providers"
    | "forge_publish_repository"
    | "forge_repo_permissions"
    | "forge_resolve_subject_url"
    | "forge_search_issues"
    | "forge_search_pull_requests"
    | "forge_sign_out"
    | "forge_update_issue"
    | "forge_update_issue_comment"
    | "forge_update_pull"
    | "git_abort_rebase"
    | "git_add_remote"
    | "git_add_submodule"
    | "git_amend_commit"
    | "git_blame"
    | "git_branch_ahead_behind"
    | "git_cancel_operation"
    | "git_checkout"
    | "git_cherry_pick"
    | "git_clone"
    | "git_close_repository"
    | "git_commit"
    | "git_commit_detail"
    | "git_conflict_file"
    | "git_continue_rebase"
    | "git_create_branch"
    | "git_create_tag"
    | "git_create_worktree"
    | "git_delete_branch"
    | "git_delete_tag"
    | "git_discard_changes"
    | "git_fetch"
    | "git_file_at_revision"
    | "git_history_chart"
    | "git_history_page"
    | "git_init_repository"
    | "git_lfs_smudge"
    | "git_lfs_status"
    | "git_list_branches_and_tags"
    | "git_list_commit_hooks"
    | "git_list_gitignore_templates"
    | "git_list_licenses"
    | "git_list_remotes"
    | "git_list_remotes_by_path"
    | "git_list_submodules"
    | "git_list_worktrees"
    | "git_lock_worktree"
    | "git_merge"
    | "git_merge_abort"
    | "git_merge_continue"
    | "git_open_diff"
    | "git_open_graph"
    | "git_open_repository"
    | "git_operation_state"
    | "git_pull"
    | "git_push"
    | "git_read_commit_hook"
    | "git_read_diff_context"
    | "git_read_diff_image"
    | "git_read_diff_range"
    | "git_read_graph_range"
    | "git_read_worktree_image"
    | "git_readme"
    | "git_remote_repo_info"
    | "git_remote_repo_info_by_path"
    | "git_remove_remote"
    | "git_remove_repository"
    | "git_remove_worktree"
    | "git_rename_branch"
    | "git_repo_icon"
    | "git_repo_icon_by_path"
    | "git_repository_snapshot"
    | "git_reset"
    | "git_resolve_conflict"
    | "git_revert"
    | "git_run_commit_hook_streamed"
    | "git_set_remote_url"
    | "git_stage_paths"
    | "git_start_rebase"
    | "git_stash_list"
    | "git_stash_pop"
    | "git_stash_push"
    | "git_status"
    | "git_unlock_worktree"
    | "git_unstage_paths"
    | "git_update_submodules"
    | "git_write_commit_hook"
    | "highlight_code"
    | "icon_refresh"
    | "icon_resolve"
    | "open_in_editor"
    | "open_in_terminal"
    | "reveal_in_file_manager"
    | "session_load"
    | "session_repositories"
    | "session_save"
    | "settings_load"
    | "settings_set";

export interface CommandArgs {
    fonts_list: {};
    forge_account: { provider: string };
    forge_begin_sign_in: { provider: string; host: string };
    forge_cancel_sign_in: {};
    forge_complete_sign_in: {};
    forge_connections: {};
    forge_create_issue: { provider: string; owner: string; repo: string; title: string; body?: string; labels?: string[] };
    forge_create_issue_comment: { provider: string; owner: string; repo: string; number: number; body: string };
    forge_create_pull: { provider: string; owner: string; repo: string; body: CreatePullRequestBody };
    forge_delete_issue_comment: { provider: string; owner: string; repo: string; commentId: number };
    forge_get_check_run: { provider: string; owner: string; repo: string; checkRunId: number };
    forge_get_check_run_log: { provider: string; owner: string; repo: string; checkRunId: number };
    forge_get_issue: { provider: string; owner: string; repo: string; number: number };
    forge_get_pull: { provider: string; owner: string; repo: string; number: number };
    forge_list_check_runs: { provider: string; owner: string; repo: string; sha: string };
    forge_list_commit_statuses: { provider: string; owner: string; repo: string; sha: string };
    forge_list_issue_comments: { provider: string; owner: string; repo: string; number: number };
    forge_list_issue_events: { provider: string; owner: string; repo: string; number: number };
    forge_list_issues: { provider: string; owner: string; repo: string; issueState?: string; labels?: string[]; page?: number };
    forge_list_labels: { provider: string; owner: string; repo: string };
    forge_list_notifications: { provider: string; page?: number };
    forge_list_orgs: { provider: string };
    forge_list_pull_commits: { provider: string; owner: string; repo: string; number: number };
    forge_list_pull_requests: { provider: string; owner: string; repo: string; pullState?: string; labels?: string[]; page?: number };
    forge_list_pull_review_comments: { provider: string; owner: string; repo: string; number: number };
    forge_list_pull_reviews: { provider: string; owner: string; repo: string; number: number };
    forge_list_workflow_runs: { provider: string; owner: string; repo: string; sha: string };
    forge_mark_all_notifications_read: { provider: string };
    forge_mark_notification_read: { provider: string; threadId: string };
    forge_merge_pull: { provider: string; owner: string; repo: string; number: number; body: MergePullRequestBody };
    forge_providers: {};
    forge_publish_repository: { repoId: number; provider?: string; owner?: string; name: string; description?: string; private?: boolean; expectedGeneration?: number };
    forge_repo_permissions: { provider: string; owner: string; repo: string };
    forge_resolve_subject_url: { provider: string; subjectUrl: string };
    forge_search_issues: { provider: string; page?: number };
    forge_search_pull_requests: { provider: string; page?: number };
    forge_sign_out: { provider: string; host: string };
    forge_update_issue: { provider: string; owner: string; repo: string; number: number; body: UpdateIssueBody };
    forge_update_issue_comment: { provider: string; owner: string; repo: string; commentId: number; body: string };
    forge_update_pull: { provider: string; owner: string; repo: string; number: number; body: UpdatePullRequestBody };
    git_abort_rebase: { repoId: number; expectedGeneration?: number };
    git_add_remote: { repoId: number; name: string; url: string; expectedGeneration?: number };
    git_add_submodule: { repoId: number; url: string; path: string; branch?: string; expectedGeneration?: number };
    git_amend_commit: { repoId: number; message?: string; expectedGeneration?: number };
    git_blame: { repoId: number; path: string; revision?: string };
    git_branch_ahead_behind: { repoId: number; head: string; base: string };
    git_cancel_operation: { operationId: number };
    git_checkout: { repoId: number; target: string; force?: boolean; mode?: CheckoutMode; paths?: string[]; expectedGeneration?: number };
    git_cherry_pick: { repoId: number; target: string; expectedGeneration?: number };
    git_clone: { request: CloneRequest; onEvent: Channel<CloneEvent> };
    git_close_repository: { repoId: number };
    git_commit: { repoId: number; message: string; stageAll?: boolean; allowEmpty?: boolean; authorName?: string; authorEmail?: string; expectedGeneration?: number; onEvent: Channel<CommitHookEvent> };
    git_commit_detail: { repoId: number; revision: string; detectRenames?: boolean };
    git_conflict_file: { repoId: number; path: string; stage: number };
    git_continue_rebase: { repoId: number; expectedGeneration?: number };
    git_create_branch: { repoId: number; name: string; startPoint?: string; force?: boolean; checkout?: boolean; expectedGeneration?: number };
    git_create_tag: { repoId: number; name: string; target?: string; message?: string; force?: boolean; expectedGeneration?: number };
    git_create_worktree: { repoId: number; name: string; path?: string; startPoint?: string; expectedGeneration?: number };
    git_delete_branch: { repoId: number; name: string; force?: boolean; expectedGeneration?: number };
    git_delete_tag: { repoId: number; name: string; expectedGeneration?: number };
    git_discard_changes: { repoId: number; paths: string[]; all?: boolean; expectedGeneration?: number };
    git_fetch: { repoId: number; remote?: string; prune?: boolean; credential?: CredentialRequest };
    git_file_at_revision: { repoId: number; revision: string; path: string };
    git_history_chart: { repoId: number; query?: HistoryChartQuery };
    git_history_page: { repoId: number; query?: HistoryPageQuery };
    git_init_repository: { request: CreateRepositoryRequest };
    git_lfs_smudge: { repoId: number; expectedGeneration?: number };
    git_lfs_status: { repoId: number };
    git_list_branches_and_tags: { repoId: number };
    git_list_commit_hooks: { repoId: number };
    git_list_gitignore_templates: {};
    git_list_licenses: {};
    git_list_remotes: { repoId: number };
    git_list_remotes_by_path: { path: string };
    git_list_submodules: { repoId: number };
    git_list_worktrees: { repoId: number };
    git_lock_worktree: { repoId: number; name: string; reason?: string; expectedGeneration?: number };
    git_merge: { repoId: number; target: string; fastForwardOnly?: boolean; noFastForward?: boolean; message?: string; expectedGeneration?: number };
    git_merge_abort: { repoId: number; expectedGeneration?: number };
    git_merge_continue: { repoId: number; message?: string; expectedGeneration?: number };
    git_open_diff: { repoId: number; request?: DiffRequest; onEvent: Channel<DiffEvent> };
    git_open_graph: { repoId: number; query?: GraphQuery; onEvent: Channel<GraphEvent> };
    git_open_repository: { request: OpenRepositoryRequest };
    git_operation_state: { repoId: number };
    git_pull: { repoId: number; remote?: string; branch?: string; fastForwardOnly?: boolean; rebase?: boolean; credential?: CredentialRequest; expectedGeneration?: number };
    git_push: { repoId: number; remote?: string; refspecs?: string[]; force?: boolean; setUpstream?: boolean; credential?: CredentialRequest };
    git_read_commit_hook: { repoId: number; hook: string };
    git_read_diff_context: { operationId: number; sectionId: number; oldStart: number; newStart: number; count: number };
    git_read_diff_image: { operationId: number; sectionId: number };
    git_read_diff_range: { operationId: number; startRow: number; maxRows: number };
    git_read_graph_range: { operationId: number; startRow: number; maxRows: number };
    git_read_worktree_image: { repoId: number; path: string };
    git_readme: { repoId: number };
    git_remote_repo_info: { repoId: number };
    git_remote_repo_info_by_path: { path: string };
    git_remove_remote: { repoId: number; name: string; expectedGeneration?: number };
    git_remove_repository: { path: string; moveToTrash: boolean };
    git_remove_worktree: { repoId: number; name: string; force?: boolean; expectedGeneration?: number };
    git_rename_branch: { repoId: number; oldName: string; newName: string; force?: boolean; expectedGeneration?: number };
    git_repo_icon: { repoId: number };
    git_repo_icon_by_path: { path: string };
    git_repository_snapshot: { repoId: number };
    git_reset: { repoId: number; kind: ResetKind; target: string; expectedGeneration?: number };
    git_resolve_conflict: { repoId: number; path: string; side: ResolveSide; expectedGeneration?: number };
    git_revert: { repoId: number; target: string; parentIndex?: number; expectedGeneration?: number };
    git_run_commit_hook_streamed: { repoId: number; hook: string; onEvent: Channel<HookOutputChunk> };
    git_set_remote_url: { repoId: number; name: string; url: string; expectedGeneration?: number };
    git_stage_paths: { repoId: number; paths: string[]; all?: boolean; expectedGeneration?: number };
    git_start_rebase: { repoId: number; upstream: string; onto?: string; expectedGeneration?: number };
    git_stash_list: { repoId: number };
    git_stash_pop: { repoId: number; index: number; action?: StashAction; expectedGeneration?: number };
    git_stash_push: { repoId: number; message?: string; includeUntracked?: boolean; keepIndex?: boolean; paths?: string[]; expectedGeneration?: number };
    git_status: { repoId: number; options?: StatusOptions };
    git_unlock_worktree: { repoId: number; name: string; expectedGeneration?: number };
    git_unstage_paths: { repoId: number; paths: string[]; expectedGeneration?: number };
    git_update_submodules: { repoId: number; names?: string[]; recursive?: boolean; init?: boolean; expectedGeneration?: number };
    git_write_commit_hook: { repoId: number; hook: string; content: string };
    highlight_code: { language: string; text: string };
    icon_refresh: { remoteUrl: string };
    icon_resolve: { remoteUrl: string };
    open_in_editor: { path: string; relativePath?: string };
    open_in_terminal: { path: string };
    reveal_in_file_manager: { path: string; relativePath?: string };
    session_load: {};
    session_repositories: { paths: string[] };
    session_save: { doc: SessionDocument };
    settings_load: {};
    settings_set: { key: string; value: SettingValue };
}

export interface CommandResults {
    fonts_list: string[];
    forge_account: AccountProfile | null;
    forge_begin_sign_in: AuthChallenge;
    forge_cancel_sign_in: void;
    forge_complete_sign_in: ConnectionProfile;
    forge_connections: ConnectionProfile[];
    forge_create_issue: IssueDetail;
    forge_create_issue_comment: IssueComment;
    forge_create_pull: PullRequestDetail;
    forge_delete_issue_comment: void;
    forge_get_check_run: CheckRunDetail;
    forge_get_check_run_log: CheckRunLog;
    forge_get_issue: IssueDetail;
    forge_get_pull: PullRequestDetail;
    forge_list_check_runs: CheckRun[];
    forge_list_commit_statuses: CommitStatus[];
    forge_list_issue_comments: IssueComment[];
    forge_list_issue_events: IssueEvent[];
    forge_list_issues: SearchIssuePage;
    forge_list_labels: Label[];
    forge_list_notifications: NotificationPage;
    forge_list_orgs: Org[];
    forge_list_pull_commits: PullRequestCommit[];
    forge_list_pull_requests: SearchPullRequestPage;
    forge_list_pull_review_comments: PullRequestReviewComment[];
    forge_list_pull_reviews: PullRequestReview[];
    forge_list_workflow_runs: WorkflowRun[];
    forge_mark_all_notifications_read: void;
    forge_mark_notification_read: void;
    forge_merge_pull: MergePullRequestResult;
    forge_providers: ProviderDescriptor[];
    forge_publish_repository: PublishResult;
    forge_repo_permissions: RepoPermissions;
    forge_resolve_subject_url: string | null;
    forge_search_issues: SearchIssuePage;
    forge_search_pull_requests: SearchPullRequestPage;
    forge_sign_out: void;
    forge_update_issue: IssueDetail;
    forge_update_issue_comment: IssueComment;
    forge_update_pull: PullRequestDetail;
    git_abort_rebase: WorkflowOutcome;
    git_add_remote: void;
    git_add_submodule: void;
    git_amend_commit: CommitSummary;
    git_blame: BlameResult;
    git_branch_ahead_behind: AheadBehind;
    git_cancel_operation: boolean;
    git_checkout: void;
    git_cherry_pick: WorkflowOutcome;
    git_clone: number;
    git_close_repository: boolean;
    git_commit: CommitExecution;
    git_commit_detail: CommitDetail;
    git_conflict_file: ConflictFile;
    git_continue_rebase: WorkflowOutcome;
    git_create_branch: BranchInfo;
    git_create_tag: TagInfo;
    git_create_worktree: WorktreeInfo;
    git_delete_branch: void;
    git_delete_tag: void;
    git_discard_changes: void;
    git_fetch: void;
    git_file_at_revision: FileContent;
    git_history_chart: HistoryChart;
    git_history_page: HistoryPage;
    git_init_repository: CreateRepositoryResult;
    git_lfs_smudge: number;
    git_lfs_status: LfsStatus;
    git_list_branches_and_tags: RepoListing;
    git_list_commit_hooks: GitHook[];
    git_list_gitignore_templates: GitignoreTemplateInfo[];
    git_list_licenses: LicenseTemplateInfo[];
    git_list_remotes: RemoteInfo[];
    git_list_remotes_by_path: RemoteInfo[];
    git_list_submodules: SubmoduleInfo[];
    git_list_worktrees: WorktreeInfo[];
    git_lock_worktree: void;
    git_merge: WorkflowOutcome;
    git_merge_abort: WorkflowOutcome;
    git_merge_continue: WorkflowOutcome;
    git_open_diff: number;
    git_open_graph: number;
    git_open_repository: OpenedRepository;
    git_operation_state: OperationState;
    git_pull: WorkflowOutcome;
    git_push: PushOutcome[];
    git_read_commit_hook: HookContent;
    git_read_diff_context: DiffContextResult;
    git_read_diff_image: DiffImage | null;
    git_read_diff_range: RangeResult;
    git_read_graph_range: GraphRangeResult;
    git_read_worktree_image: string | null;
    git_readme: ReadmeContent | null;
    git_remote_repo_info: RemoteRepoInfo;
    git_remote_repo_info_by_path: RemoteRepoInfo;
    git_remove_remote: void;
    git_remove_repository: RemoveRepositoryResult;
    git_remove_worktree: void;
    git_rename_branch: BranchInfo;
    git_repo_icon: CachedIcon | null;
    git_repo_icon_by_path: CachedIcon | null;
    git_repository_snapshot: RepoSnapshot;
    git_reset: void;
    git_resolve_conflict: void;
    git_revert: WorkflowOutcome;
    git_run_commit_hook_streamed: HookRunResult;
    git_set_remote_url: void;
    git_stage_paths: void;
    git_start_rebase: WorkflowOutcome;
    git_stash_list: StashEntry[];
    git_stash_pop: StashEntry[];
    git_stash_push: ObjectId;
    git_status: StatusReport;
    git_unlock_worktree: void;
    git_unstage_paths: void;
    git_update_submodules: void;
    git_write_commit_hook: void;
    highlight_code: HighlightedSnippet;
    icon_refresh: CachedIcon | null;
    icon_resolve: CachedIcon | null;
    open_in_editor: void;
    open_in_terminal: void;
    reveal_in_file_manager: void;
    session_load: SessionDocument | null;
    session_repositories: SessionRepository[];
    session_save: void;
    settings_load: SettingsSnapshot;
    settings_set: ValuesSnapshot;
}
