export type Oid = string;

export interface Signature {
    name: string;
    email: string;
    timeSeconds: number;
    timeOffsetMinutes: number;
}

export type HeadState =
    | { state: "attached"; branch: string; target: Oid }
    | { state: "detached"; target: Oid }
    | { state: "unborn"; branch: string };

export type ShaKind = "sha1" | "sha256";

export interface RepoSnapshot {
    id: number;
    generation: number;
    workdir?: string;
    gitDir: string;
    head: HeadState;
    shaKind: ShaKind;
}

export interface OpenedRepository {
    id: number;
    snapshot: RepoSnapshot;
}

export interface CreateRepositoryRequest {
    parentDirectory: string;
    /** Ignored when initInPlace is set. */
    name: string;
    /** Initializes directly in parentDirectory instead of a new subfolder. */
    initInPlace?: boolean;
    readme: boolean;
    gitignoreTemplate?: string | null;
    license?: string | null;
}

/** Created repository plus the canonical path to open it by later. */
export type CreateRepositoryResult = OpenedRepository & { path: string };

/**
 * Outcome of removing a repository: the closed backend session id (absent
 * when the repo had no open session) and the canonical repo root the
 * removal and optional trash applied to.
 */
export interface RemoveRepositoryResult {
    removedRepoId?: number;
    repoRoot: string;
}

export interface GitignoreTemplateInfo {
    id: string;
    label: string;
}

export interface LicenseTemplateInfo {
    id: string;
    name: string;
    description: string;
}

export type ClonePhase = "counting" | "receiving" | "resolving" | "checkingOut";

export interface CloneProgress {
    phase: ClonePhase;
    /** 0..1; 0 while the phase is indeterminate (counting, checkout). */
    progress: number;
    objectsReceived: number;
    objectsTotal: number | null;
    receivedBytes: number;
}

/**
 * Stream of updates from an in-flight clone. Terminal states mirror
 * DiffEvent so every streamed operation shares one lifecycle model.
 */
export type CloneEvent =
    | ({ event: "progress"; operationId: number } & CloneProgress)
    | { event: "completed"; operationId: number; repoPath: string }
    | { event: "failed"; operationId: number; code: string; message: string }
    | { event: "cancelled"; operationId: number };

/**
 * Emitted by the backend whenever a repository's generation advances
 * (filesystem watcher debounce or a successful mutation).
 */
export interface RepositoryInvalidation {
    repoId: number;
    generation: number;
    reason: string;
}

export type ChangeKind =
    | "added"
    | "deleted"
    | "modified"
    | "typeChanged"
    | "renamed"
    | "copied"
    | "conflicted"
    | "untracked";

export type ChangeSide = "index" | "worktree";

/**
 * One selectable change. A path can appear twice, once staged and once
 * unstaged, and each row has its own stable `id`
 * (`index:<path>` / `worktree:<path>`) for selection and diff requests.
 */
export interface StatusEntry {
    id: string;
    side: ChangeSide;
    path: string;
    oldPath?: string;
    kind: ChangeKind;
}

export interface ConflictSide {
    stage: number;
    id: Oid;
    mode: number;
}

export interface ConflictEntry {
    path: string;
    sides: ConflictSide[];
}

export interface StatusReport {
    snapshotId: number;
    generation: number;
    entries: StatusEntry[];
    conflicts: ConflictEntry[];
}

export interface StatusOptions {
    includeIgnored?: boolean;
    includeUntracked?: boolean;
    recurseUntrackedDirs?: boolean;
}

export type SearchMatchField = "summary" | "author";

export interface SearchMatchRange {
    field: SearchMatchField;
    start: number;
    length: number;
}

export interface CommitSummary {
    id: Oid;
    treeId: Oid;
    parentIds: Oid[];
    summaryLine: string;
    message: string;
    author: Signature;
    committer: Signature;
    /** Files touched vs the first parent. */
    filesChanged: number;
    additions: number;
    deletions: number;
    /** Tag names pointing at this commit. */
    tags: string[];
    /** Fuzzy-match ranges for the current search, in char offsets. */
    matchRanges?: SearchMatchRange[];
}

export interface FileChangeStat {
    path: string;
    oldPath?: string;
    kind: ChangeKind;
    additions: number;
    deletions: number;
    binary: boolean;
}

export interface CommitDetail {
    summary: CommitSummary;
    files: FileChangeStat[];
    totalAdditions: number;
    totalDeletions: number;
}

export interface HistoryPageQuery {
    revision?: string;
    limit?: number;
    skip?: number;
    excludeReachableFrom?: string[];
    path?: string;
    /** Case-insensitive filter against commit message and author: each term must substring-match or fuzzy-match inside one word. */
    search?: string;
}

export interface HistoryPage {
    snapshotId: number;
    generation: number;
    commits: CommitSummary[];
    hasMore: boolean;
}

export type ChartBucketSize =
    | "hour"
    | "quarterDay"
    | "halfDay"
    | "day"
    | "twoDays"
    | "halfWeek"
    | "week"
    | "month"
    | "quarter";

export interface HistoryChartQuery {
    revision?: string;
    excludeReachableFrom?: string[];
    /** Fixed bucket unit; omit to auto-select from time since first commit
     * (10y+ monthly, 6y+ weekly, 3y+ half-weekly, 1y+ two-day, 180d+ daily,
     * 90d+ half-daily, 14d+ quarter-daily, younger hourly). */
    bucket?: ChartBucketSize;
    /** Walk cap; older commits past this are not charted. */
    maxCommits?: number;
    /** Count merge commits toward `commits`; their line deltas never count. */
    includeMerges?: boolean;
}

export interface ChartBucket {
    /** UTC epoch seconds of the bucket's inclusive start. */
    startSeconds: number;
    /** UTC epoch seconds of the bucket's exclusive end. */
    endSeconds: number;
    commits: number;
    additions: number;
    deletions: number;
}

export interface HistoryChart {
    snapshotId: number;
    generation: number;
    /** Resolved unit after applying auto-selection. */
    bucketSize: ChartBucketSize;
    /** Chronological; gaps between first and last activity are zero-filled. */
    buckets: ChartBucket[];
    totalCommits: number;
    /** True when the walk hit the commit cap and older history is not shown. */
    truncated: boolean;
}

export interface UpstreamRef {
    remote: string;
    branch: string;
    ahead: number;
    behind: number;
    /** Commit the remote-tracking ref points at; absent when unborn. */
    target?: Oid;
}

export interface BranchInfo {
    name: string;
    target: Oid;
    isHead: boolean;
    upstream?: UpstreamRef;
}

export interface TagInfo {
    name: string;
    target: Oid;
    tagObject?: Oid;
    message?: string;
    tagger?: Signature;
}

export interface RepoListing {
    branches: BranchInfo[];
    tags: TagInfo[];
}

export interface WorktreeInfo {
    /** `main` is the primary tree; linked trees use their libgit2 key. */
    name: string;
    path: string;
    /** Short branch name checked out in this tree, when attached. */
    branch?: string;
    isCurrent: boolean;
    lockedBy?: string;
    /** Checkout directory no longer exists on disk. */
    isPrunable: boolean;
}

/** Raw file content at a revision. `data` is a JSON array of byte values. */
export interface FileContent {
    path: string;
    revision: Oid;
    data: number[];
    binary: boolean;
    size: number;
}

export interface RemoteInfo {
    name: string;
    url?: string;
    pushUrl?: string;
}

export interface PushOutcome {
    reference: string;
    accepted: boolean;
    reason?: string;
}

export interface StashEntry {
    index: number;
    message: string;
    commit: string;
}

export type WorkflowOutcome =
    | { outcome: "alreadyUpToDate" }
    | { outcome: "fastForwarded"; head: string }
    | { outcome: "merged"; commit: string }
    | { outcome: "conflicted"; paths: string[] }
    | { outcome: "started"; totalSteps: number }
    | { outcome: "progressed"; remaining: number }
    | { outcome: "finished"; commit?: string }
    | { outcome: "aborted" };

export type OperationKind = "none" | "merge";

export interface OperationState {
    kind: OperationKind;
    message?: string;
    conflictPaths: string[];
    heads: string[];
}

export type ResolveSide = "ours" | "theirs";

export interface ConflictStyle {
    light: string;
    dark: string;
    b?: boolean;
    i?: boolean;
    u?: boolean;
}

export interface ConflictFile {
    path: string;
    stage: number;
    revision: Oid;
    binary: boolean;
    rows: DiffRow[];
    styles: ConflictStyle[];
    truncated: boolean;
}

/** A finished commit together with every hook that executed around it. */
export interface CommitExecution {
    summary: CommitSummary;
    hookRuns: HookRunResult[];
}

export type ResetKind = "soft" | "mixed" | "hard";

/** A commit-lifecycle hook script found on disk (`core.hooksPath` aware). */
export interface GitHook {
    name: string;
    path: string;
    executable: boolean;
}

/** Outcome of one manual hook run; failure is a value, not an error. */
export interface HookRunResult {
    hook: string;
    exitCode: number | null;
    success: boolean;
    stdout: string;
    stderr: string;
    durationMs: number;
}

/** A commit hook script as editable text; hooks missing on disk report
 * `exists: false` with empty content so the editor can create them. */
export interface HookContent {
    hook: string;
    path: string;
    exists: boolean;
    content: string;
}

/**
 * Externally tagged exactly like the Rust `CredentialKind` enum:
 * `"default"` or `{ usernamePassword: {...} }` etc.
 */
export type CredentialRequest =
    | "default"
    | { usernamePassword: { username: string; password: string } }
    | {
          sshKey: {
              username: string;
              keyPath: string;
              passphrase?: string;
          };
      }
    | {
          inMemorySshKey: {
              username: string;
              keyPem: string;
              passphrase?: string;
          };
      };

/**
 * Externally tagged like Rust `DiffComparison`. Unit variants are plain
 * strings; struct variants are single-key objects.
 */
export type DiffComparison =
    | "workingTree"
    | "unstaged"
    | "staged"
    | { treeToTree: { old: string; new: string } }
    | { commitToParent: { commit: string } };

export interface DiffRequest {
    comparison?: DiffComparison;
    contextLines?: number;
    interhunkLines?: number;
    detectRenames?: boolean;
    detectCopies?: boolean;
    paths?: string[];
    ignoreWhitespace?: boolean;
}

export type DiffRowKind =
    | "fileHeader"
    | "hunkHeader"
    | "context"
    | "addition"
    | "deletion"
    | "binaryNotice";

export interface DiffRow {
    kind: DiffRowKind;
    oldLineno?: number;
    newLineno?: number;
    content: string;
    rawHex?: string;
    /** Flat [start, len, styleId] triples covering non-plain segments. */
    spans?: number[];
}

export type SectionKind =
    | "added"
    | "deleted"
    | "modified"
    | "renamed"
    | "copied"
    | "typeChanged"
    | "conflicted";

export interface SectionMeta {
    sectionId: number;
    path: string;
    oldPath?: string;
    kind: SectionKind;
    binary: boolean;
    image?: boolean;
    complete: boolean;
}

/** One resolved highlighting style covering both app themes. */
export interface SyntaxStyle {
    light: string;
    dark: string;
    b?: boolean;
    i?: boolean;
    u?: boolean;
}

export type DiffEvent =
    | {
          event: "started";
          operationId: number;
          snapshotId: number;
          generation: number;
          sections: SectionMeta[];
          estimatedTotalRows: number;
      }
    | {
          event: "sectionLayout";
          operationId: number;
          sectionId: number;
          startRow: number;
          rowCount: number;
      }
    | {
          event: "chunk";
          operationId: number;
          sectionId: number;
          rowStart: number;
          rows: DiffRow[];
          /** Per-row span triples aligned with `rows`; null entries are plain. */
          spansByRow?: (number[] | null)[];
          /** Style-table delta; spans' third element indexes into the
           * per-section table these append to. */
          styles?: SyntaxStyle[];
      }
    | { event: "layoutReady"; operationId: number; totalRows: number }
    | {
          event: "completed";
          operationId: number;
          totalRows: number;
          additions: number;
          deletions: number;
          durationMs: number;
      }
    | { event: "failed"; operationId: number; code: string; message: string }
    | { event: "cancelled"; operationId: number };

export interface RangeRow extends DiffRow {
    sectionId: number;
    path?: string;
    sectionKind: SectionKind;
}

export interface DiffImageSide {
    data: number[];
    mimeType: string;
}

export interface DiffImage {
    old?: DiffImageSide;
    new?: DiffImageSide;
}

export interface RangeResult {
    rows: RangeRow[];
    nextCursor: number;
    hasMore: boolean;
    knownTotalRows: number;
    complete: boolean;
}

export interface GraphQuery {
    revision?: string;
    excludeReachableFrom?: string[];
}

export interface GraphEdge {
    fromLane: number;
    toLane: number;
}

export type GraphRowKind = "commit" | "merge" | "root";

export interface GraphRow {
    /** Absolute position in the walk; stable for the operation's lifetime. */
    index: number;
    id: Oid;
    lane: number;
    edges: GraphEdge[];
    kind: GraphRowKind;
    summaryLine: string;
    authorName: string;
    authorEmail: string;
    timeSeconds: number;
    tags: string[];
    /** Branch decorations pointing at this commit. */
    refs: string[];
}

export type GraphEvent =
    | {
          event: "started";
          operationId: number;
          snapshotId: number;
          generation: number;
      }
    | {
          event: "chunk";
          operationId: number;
          rowStart: number;
          rows: GraphRow[];
      }
    | { event: "completed"; operationId: number; totalRows: number }
    | { event: "failed"; operationId: number; code: string; message: string }
    | { event: "cancelled"; operationId: number };

export interface GraphRangeResult {
    rows: GraphRow[];
    nextCursor: number;
    hasMore: boolean;
    knownTotalRows: number;
    complete: boolean;
}

export interface SerializedError {
    code: string;
    message: string;
    retryable: boolean;
    detail?: string;
}

export interface SelectOption {
    value: string;
    label: string;
}

/** Raw setting payload as it crosses the IPC boundary. */
export type SettingValue = boolean | number | string | string[];

interface SettingDefinitionBase {
    /** Stable camelCase key; also the key inside the values file. */
    key: string;
    label: string;
    description?: string;
    defaultValue: SettingValue;
    /** Hidden settings are persisted but never rendered in the UI. */
    hidden: boolean;
}

/**
 * Discriminated union matching Rust's flattened `SettingType`. Renderers
 * switch on `kind` to pick a control.
 */
export type SettingDefinition =
    | (SettingDefinitionBase & { kind: "boolean" })
    | (SettingDefinitionBase & { kind: "select"; options: SelectOption[] })
    | (SettingDefinitionBase & {
          kind: "number";
          min: number | null;
          max: number | null;
          step: number | null;
      })
    | (SettingDefinitionBase & { kind: "string" })
    | (SettingDefinitionBase & { kind: "multilineString" })
    | (SettingDefinitionBase & { kind: "shortcut" })
    | (SettingDefinitionBase & { kind: "stringList" });

export interface SettingsSection {
    id: string;
    title: string;
    settings: SettingDefinition[];
}

export interface SettingsTab {
    id: string;
    label: string;
    sections: SettingsSection[];
}

export interface SettingsSchema {
    tabs: SettingsTab[];
}

export interface ValuesSnapshot {
    /**
     * Merged stored-or-default values for every schema key. Shape matches
     * the generated `SettingsValues` interface exactly.
     */
    values: Record<string, SettingValue>;
    /** Keys explicitly present in the persisted file. */
    savedKeys: string[];
}

export interface SettingsSnapshot extends ValuesSnapshot {
    schema: SettingsSchema;
}

/** Icon resolved into a data URL: a worktree icon file, or a remote
 * owner/org avatar. */
export interface CachedIcon {
    key: string;
    /** `data:<type>;base64,...` ready for `<img src>`. */
    dataUrl: string;
    contentType: string;
    /** When the cached bytes were fetched (epoch ms). */
    fetchedAtMs: number;
}

/** Non-secret profile of the connected GitHub account; the access token
 * never leaves the Rust side (it lives in the OS keychain). */
export interface AccountProfile {
    login: string;
    name: string | null;
    avatarUrl: string;
    htmlUrl: string;
    scopes: string[];
    /** Epoch ms when the connection was established. */
    connectedAtMs: number;
}

/** User-facing half of an in-progress device-flow sign-in. */
export interface DeviceFlowStart {
    userCode: string;
    verificationUri: string;
    expiresInSecs: number;
}

export interface GithubOrg {
    login: string;
    avatarUrl?: string;
}

/** One GitHub notification thread as projected by the backend. */
export interface GithubNotification {
    id: string;
    unread: boolean;
    reason: string;
    subjectTitle: string;
    subjectType: string;
    repoFullName: string;
    htmlUrl?: string | null;
    /** Subject API URL, for resolving types without a static web mapping. */
    subjectUrl?: string | null;
    updatedAt: string;
}

/** One 1-based page of the inbox; `hasMore` derives from the response
 * `Link` header. */
export interface NotificationPage {
    notifications: GithubNotification[];
    page: number;
    hasMore: boolean;
}

export interface PublishResult {
    fullName: string;
    htmlUrl: string;
    defaultBranch?: string | null;
}

/** A tab as persisted in the session document. `repoId` is process-local
 * and remapped on restore after repositories are reopened. */
export interface PersistedTab {
    tabId: string;
    title: string;
    lastResolvedHref?: string;
    repoPath?: string;
    repoId?: number;
    /** Title-badge registry key; resolved back to a component on restore. */
    titleBadge?: string;
}

/** The whole UI state, persisted by the backend to disk. The Rust side
 * round-trips the document as opaque JSON, so additive optional fields are
 * tolerated by older builds. */
/** Info fetched from a remote or local README for homepage cards. */
export interface RemoteRepoInfo {
    description?: string;
    /** Source of the description: remote API or local README. */
    source?: "remote" | "readme" | "none";
    /** Optional star count for recognized public remotes. */
    stars?: number;
    /** Optional fork count for recognized public remotes. */
    forks?: number;
    /** Local worktree language shares, largest first. */
    languages?: LanguageShare[];
    /** When the info was fetched (epoch ms). */
    fetchedAtMs?: number;
}

/** One language's byte share of a repo's worktree. */
export interface LanguageShare {
    language: string;
    /** Hex color from the github-colors dataset, or empty when none. */
    color: string;
    percent: number;
}

export interface SessionDocument {
    version: number;
    tabs: PersistedTab[];
    activeTabId: string | null;
    /** Repo path -> epoch ms of the last completed fetch. */
    lastFetched?: Record<string, number>;
    /**
     * Explicitly added repositories, independent of open tabs; restored so
     * repos survive restarts even with no tab referencing them.
     */
    repositories?: { path: string; addedAt: number }[];
}
