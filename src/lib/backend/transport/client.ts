import { Channel } from "@tauri-apps/api/core";

import type {
    AccountProfile,
    BranchInfo,
    CachedIcon,
    CloneEvent,
    CommitDetail,
    CommitExecution,
    CommitSummary,
    ConflictFile,
    CreateRepositoryRequest,
    CreateRepositoryResult,
    CredentialRequest,
    DeviceFlowStart,
    DiffEvent,
    DiffImage,
    DiffRequest,
    FileContent,
    GithubIssueComment,
    GithubIssueDetail,
    GithubIssueEvent,
    GithubIssueListItem,
    GithubOrg,
    GithubRepoPermissions,
    GraphEvent,
    HighlightedSnippet,
    GraphQuery,
    GraphRangeResult,
    GitHook,
    GitignoreTemplateInfo,
    HistoryChart,
    HistoryChartQuery,
    HistoryPage,
    HistoryPageQuery,
    HookContent,
    HookRunResult,
    LicenseTemplateInfo,
    NotificationPage,
    Oid,
    OpenedRepository,
    OperationState,
    PublishResult,
    PushOutcome,
    RangeResult,
    RepoListing,
    RepoSnapshot,
    RemoteInfo,
    RemoteRepoInfo,
    RepositoryInvalidation,
    RemoveRepositoryResult,
    ResetKind,
    ResolveSide,
    SessionDocument,
    SettingValue,
    SettingsSnapshot,
    StashEntry,
    StatusOptions,
    StatusReport,
    TagInfo,
    UpdateIssueBody,
    ValuesSnapshot,
    WorkflowOutcome,
    WorktreeInfo,
} from "../protocol";
import { invokeCommand } from "./invoke";
import { asResult } from "./result";
import type { Result } from "./result";

/**
 * The single place where raw Tauri command names exist. Everything else in
 * the app talks to this typed client.
 *
 * Every method returns a `Result`: the client NEVER rejects, so callers
 * are forced by the compiler to narrow on `ok` before using a value.
 */
function createRawBackendClient() {
    return {
        repositories: {
            open: (path: string) =>
                invokeCommand<OpenedRepository>("git_open_repository", {
                    args: { request: { path } },
                }),
            close: (repoId: number) =>
                invokeCommand<boolean>("git_close_repository", {
                    args: { repoId },
                }),
            remove: (path: string, moveToTrash: boolean) =>
                invokeCommand<RemoveRepositoryResult>("git_remove_repository", {
                    args: { path, moveToTrash },
                }),
            snapshot: (repoId: number) =>
                invokeCommand<RepoSnapshot>("git_repository_snapshot", {
                    args: { repoId },
                }),
            create: (request: CreateRepositoryRequest) =>
                invokeCommand<CreateRepositoryResult>("git_init_repository", {
                    args: { request },
                }),
            gitignoreTemplates: () =>
                invokeCommand<GitignoreTemplateInfo[]>(
                    "git_list_gitignore_templates"
                ),
            licenses: () =>
                invokeCommand<LicenseTemplateInfo[]>("git_list_licenses"),
        },

        changes: {
            status: (repoId: number, options?: StatusOptions) =>
                invokeCommand<StatusReport>("git_status", {
                    args: { repoId, options },
                }),
            stagePaths: (
                repoId: number,
                paths: string[],
                all: boolean | undefined,
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_stage_paths", {
                    args: { repoId, paths, all, expectedGeneration },
                }),
            unstagePaths: (
                repoId: number,
                paths: string[],
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_unstage_paths", {
                    args: { repoId, paths, expectedGeneration },
                }),
            discardChanges: (
                repoId: number,
                paths: string[],
                all: boolean | undefined,
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_discard_changes", {
                    args: { repoId, paths, all, expectedGeneration },
                }),
        },

        hooks: {
            list: (repoId: number) =>
                invokeCommand<GitHook[]>("git_list_commit_hooks", {
                    args: { repoId },
                }),
            run: (repoId: number, hook: string) =>
                invokeCommand<HookRunResult>("git_run_commit_hook", {
                    args: { repoId, hook },
                }),
            read: (repoId: number, hook: string) =>
                invokeCommand<HookContent>("git_read_commit_hook", {
                    args: { repoId, hook },
                }),
            write: (repoId: number, hook: string, content: string) =>
                invokeCommand<void>("git_write_commit_hook", {
                    args: { repoId, hook, content },
                }),
        },

        diff: {
            /**
             * Opens a streaming diff session. Returns the operation id immediately;
             * events arrive ordered on the channel until a terminal event.
             */
            open: (
                repoId: number,
                request: DiffRequest | undefined,
                onEvent: (event: DiffEvent) => void
            ): Promise<number> => {
                const channel = new Channel<DiffEvent>();
                channel.onmessage = onEvent;
                return invokeCommand<number>("git_open_diff", {
                    args: { repoId, request, onEvent: channel },
                });
            },
            readImage: (operationId: number, sectionId: number) =>
                invokeCommand<DiffImage | null>("git_read_diff_image", {
                    args: { operationId, sectionId },
                }),
            readRange: (
                operationId: number,
                startRow: number,
                maxRows: number
            ) =>
                invokeCommand<RangeResult>("git_read_diff_range", {
                    args: { operationId, startRow, maxRows },
                }),
            cancel: (operationId: number) =>
                invokeCommand<boolean>("git_cancel_operation", {
                    args: { operationId },
                }),
        },

        graph: {
            /** Opens a streaming commit-graph session; mirrors diff.open. */
            open: (
                repoId: number,
                query: GraphQuery | undefined,
                onEvent: (event: GraphEvent) => void
            ): Promise<number> => {
                const channel = new Channel<GraphEvent>();
                channel.onmessage = onEvent;
                return invokeCommand<number>("git_open_graph", {
                    args: { repoId, query, onEvent: channel },
                });
            },
            readImage: (operationId: number, sectionId: number) =>
                invokeCommand<DiffImage | null>("git_read_diff_image", {
                    args: { operationId, sectionId },
                }),
            readRange: (
                operationId: number,
                startRow: number,
                maxRows: number
            ) =>
                invokeCommand<GraphRangeResult>("git_read_graph_range", {
                    args: { operationId, startRow, maxRows },
                }),
            cancel: (operationId: number) =>
                invokeCommand<boolean>("git_cancel_operation", {
                    args: { operationId },
                }),
        },

        history: {
            page: (repoId: number, query: HistoryPageQuery = {}) =>
                invokeCommand<HistoryPage>("git_history_page", {
                    args: { repoId, query },
                }),
            chart: (repoId: number, query: HistoryChartQuery = {}) =>
                invokeCommand<HistoryChart>("git_history_chart", {
                    args: { repoId, query },
                }),
            commitDetail: (
                repoId: number,
                revision: string,
                detectRenames?: boolean
            ) =>
                invokeCommand<CommitDetail>("git_commit_detail", {
                    args: { repoId, revision, detectRenames },
                }),
            fileAtRevision: (repoId: number, revision: string, path: string) =>
                invokeCommand<FileContent>("git_file_at_revision", {
                    args: { repoId, revision, path },
                }),
        },

        refs: {
            listBranchesAndTags: (repoId: number) =>
                invokeCommand<RepoListing>("git_list_branches_and_tags", {
                    args: { repoId },
                }),
            createBranch: (
                repoId: number,
                name: string,
                startPoint: string | undefined,
                force: boolean | undefined,
                checkout: boolean | undefined,
                expectedGeneration?: number
            ) =>
                invokeCommand<BranchInfo>("git_create_branch", {
                    args: {
                        repoId,
                        name,
                        startPoint,
                        force,
                        checkout,
                        expectedGeneration,
                    },
                }),
            deleteBranch: (
                repoId: number,
                name: string,
                force: boolean | undefined,
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_delete_branch", {
                    args: { repoId, name, force, expectedGeneration },
                }),
            renameBranch: (
                repoId: number,
                oldName: string,
                newName: string,
                force: boolean | undefined,
                expectedGeneration?: number
            ) =>
                invokeCommand<BranchInfo>("git_rename_branch", {
                    args: {
                        repoId,
                        oldName,
                        newName,
                        force,
                        expectedGeneration,
                    },
                }),
            createTag: (
                repoId: number,
                name: string,
                target: string | undefined,
                message: string | undefined,
                force: boolean | undefined,
                expectedGeneration?: number
            ) =>
                invokeCommand<TagInfo>("git_create_tag", {
                    args: {
                        repoId,
                        name,
                        target,
                        message,
                        force,
                        expectedGeneration,
                    },
                }),
            deleteTag: (
                repoId: number,
                name: string,
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_delete_tag", {
                    args: { repoId, name, expectedGeneration },
                }),
        },

        mutations: {
            commit: (
                repoId: number,
                message: string,
                options: {
                    stageAll?: boolean;
                    allowEmpty?: boolean;
                    authorName?: string;
                    authorEmail?: string;
                } = {},
                expectedGeneration?: number
            ) =>
                invokeCommand<CommitExecution>("git_commit", {
                    args: { repoId, message, ...options, expectedGeneration },
                }),
            amend: (
                repoId: number,
                message?: string,
                expectedGeneration?: number
            ) =>
                invokeCommand<CommitSummary>("git_amend_commit", {
                    args: { repoId, message, expectedGeneration },
                }),
            checkout: (
                repoId: number,
                target: string,
                options: { force?: boolean; paths?: string[] } = {},
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_checkout", {
                    args: { repoId, target, ...options, expectedGeneration },
                }),
            reset: (
                repoId: number,
                kind: ResetKind,
                target: string,
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_reset", {
                    args: { repoId, kind, target, expectedGeneration },
                }),
        },

        worktrees: {
            list: (repoId: number) =>
                invokeCommand<WorktreeInfo[]>("git_list_worktrees", {
                    args: { repoId },
                }),
            create: (
                repoId: number,
                name: string,
                path: string | undefined,
                startPoint: string | undefined,
                expectedGeneration?: number
            ) =>
                invokeCommand<WorktreeInfo>("git_create_worktree", {
                    args: {
                        repoId,
                        name,
                        path,
                        startPoint,
                        expectedGeneration,
                    },
                }),
            remove: (
                repoId: number,
                name: string,
                force: boolean | undefined,
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_remove_worktree", {
                    args: { repoId, name, force, expectedGeneration },
                }),
            lock: (
                repoId: number,
                name: string,
                reason: string | undefined,
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_lock_worktree", {
                    args: { repoId, name, reason, expectedGeneration },
                }),
            unlock: (
                repoId: number,
                name: string,
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_unlock_worktree", {
                    args: { repoId, name, expectedGeneration },
                }),
        },

        workflows: {
            merge: (
                repoId: number,
                target: string,
                options: {
                    fastForwardOnly?: boolean;
                    noFastForward?: boolean;
                    message?: string;
                } = {},
                expectedGeneration?: number
            ) =>
                invokeCommand<WorkflowOutcome>("git_merge", {
                    args: { repoId, target, ...options, expectedGeneration },
                }),
            mergeContinue: (
                repoId: number,
                message?: string,
                expectedGeneration?: number
            ) =>
                invokeCommand<WorkflowOutcome>("git_merge_continue", {
                    args: { repoId, message, expectedGeneration },
                }),
            mergeAbort: (repoId: number, expectedGeneration?: number) =>
                invokeCommand<WorkflowOutcome>("git_merge_abort", {
                    args: { repoId, expectedGeneration },
                }),
            operationState: (repoId: number) =>
                invokeCommand<OperationState>("git_operation_state", {
                    args: { repoId },
                }),
            resolveConflict: (
                repoId: number,
                path: string,
                side: ResolveSide,
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_resolve_conflict", {
                    args: { repoId, path, side, expectedGeneration },
                }),
            conflictFile: (repoId: number, path: string, stage: number) =>
                invokeCommand<ConflictFile>("git_conflict_file", {
                    args: { repoId, path, stage },
                }),
            cherryPick: (
                repoId: number,
                target: string,
                expectedGeneration?: number
            ) =>
                invokeCommand<WorkflowOutcome>("git_cherry_pick", {
                    args: { repoId, target, expectedGeneration },
                }),
            revert: (
                repoId: number,
                target: string,
                parentIndex?: number,
                expectedGeneration?: number
            ) =>
                invokeCommand<WorkflowOutcome>("git_revert", {
                    args: { repoId, target, parentIndex, expectedGeneration },
                }),
            stashPush: (
                repoId: number,
                options: {
                    message?: string;
                    includeUntracked?: boolean;
                    keepIndex?: boolean;
                    paths?: string[];
                } = {},
                expectedGeneration?: number
            ) =>
                invokeCommand<Oid>("git_stash_push", {
                    args: { repoId, ...options, expectedGeneration },
                }),
            stashPop: (
                repoId: number,
                index: number,
                action?: "pop" | "apply" | "drop",
                expectedGeneration?: number
            ) =>
                invokeCommand<StashEntry[]>("git_stash_pop", {
                    args: { repoId, index, action, expectedGeneration },
                }),
            stashList: (repoId: number) =>
                invokeCommand<StashEntry[]>("git_stash_list", {
                    args: { repoId },
                }),
        },

        remoteInfo: {
            info: (repoId: number) =>
                invokeCommand<RemoteRepoInfo>("git_remote_repo_info", {
                    args: { repoId },
                }),
            byPath: (path: string) =>
                invokeCommand<RemoteRepoInfo>("git_remote_repo_info_by_path", {
                    args: { path },
                }),
        },

        remotes: {
            list: (repoId: number) =>
                invokeCommand<RemoteInfo[]>("git_list_remotes", {
                    args: { repoId },
                }),
            listByPath: (path: string) =>
                invokeCommand<RemoteInfo[]>("git_list_remotes_by_path", {
                    args: { path },
                }),
            add: (
                repoId: number,
                name: string,
                url: string,
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_add_remote", {
                    args: { repoId, name, url, expectedGeneration },
                }),
            remove: (
                repoId: number,
                name: string,
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_remove_remote", {
                    args: { repoId, name, expectedGeneration },
                }),
            setUrl: (
                repoId: number,
                name: string,
                url: string,
                expectedGeneration?: number
            ) =>
                invokeCommand<void>("git_set_remote_url", {
                    args: { repoId, name, url, expectedGeneration },
                }),
            fetch: (
                repoId: number,
                options: {
                    remote?: string;
                    prune?: boolean;
                    credential?: CredentialRequest;
                } = {}
            ) =>
                invokeCommand<void>("git_fetch", {
                    args: { repoId, ...options },
                }),
            push: (
                repoId: number,
                options: {
                    remote?: string;
                    refspecs?: string[];
                    force?: boolean;
                    setUpstream?: boolean;
                    credential?: CredentialRequest;
                } = {}
            ) =>
                invokeCommand<PushOutcome[]>("git_push", {
                    args: { repoId, ...options },
                }),
            pull: (
                repoId: number,
                options: {
                    remote?: string;
                    branch?: string;
                    fastForwardOnly?: boolean;
                    rebase?: boolean;
                    credential?: CredentialRequest;
                } = {},
                expectedGeneration?: number
            ) =>
                invokeCommand<WorkflowOutcome>("git_pull", {
                    args: { repoId, ...options, expectedGeneration },
                }),
            /**
             * Starts a clone. Resolves with the operation id immediately;
             * CloneEvent updates arrive on the callback until a terminal
             * event.
             */
            clone: (
                request: { url: string; destination: string },
                onEvent: (event: CloneEvent) => void
            ) => {
                const channel = new Channel<CloneEvent>();
                channel.onmessage = onEvent;
                return invokeCommand<number>("git_clone", {
                    args: { request, onEvent: channel },
                });
            },
        },

        operations: {
            cancel: (operationId: number) =>
                invokeCommand<boolean>("git_cancel_operation", {
                    args: { operationId },
                }),
        },

        highlight: {
            code: (language: string, text: string) =>
                invokeCommand<HighlightedSnippet>("highlight_code", {
                    args: { language, text },
                }),
        },

        github: {
            account: () =>
                invokeCommand<AccountProfile | null>("github_account"),
            beginSignIn: () =>
                invokeCommand<DeviceFlowStart>("github_begin_sign_in"),
            completeSignIn: () =>
                invokeCommand<AccountProfile>("github_complete_sign_in"),
            cancelSignIn: () => invokeCommand<void>("github_cancel_sign_in"),
            signOut: () => invokeCommand<void>("github_sign_out"),
            listOrgs: () => invokeCommand<GithubOrg[]>("github_list_orgs"),
            listNotifications: (page?: number) =>
                invokeCommand<NotificationPage>("github_list_notifications", {
                    args: { page },
                }),
            markNotificationRead: (threadId: string) =>
                invokeCommand<void>("github_mark_notification_read", {
                    args: { threadId },
                }),
            markAllNotificationsRead: () =>
                invokeCommand<void>("github_mark_all_notifications_read"),
            resolveSubjectUrl: (subjectUrl: string) =>
                invokeCommand<string | null>("github_resolve_subject_url", {
                    args: { subjectUrl },
                }),
            listIssues: (
                owner: string,
                repo: string,
                issueState?: string,
                labels?: string[]
            ) =>
                invokeCommand<GithubIssueListItem[]>("github_list_issues", {
                    args: { owner, repo, issueState, labels },
                }),
            getIssue: (owner: string, repo: string, number: number) =>
                invokeCommand<GithubIssueDetail>("github_get_issue", {
                    args: { owner, repo, number },
                }),
            listIssueComments: (owner: string, repo: string, number: number) =>
                invokeCommand<GithubIssueComment[]>(
                    "github_list_issue_comments",
                    { args: { owner, repo, number } }
                ),
            listIssueEvents: (owner: string, repo: string, number: number) =>
                invokeCommand<GithubIssueEvent[]>("github_list_issue_events", {
                    args: { owner, repo, number },
                }),
            createIssueComment: (
                owner: string,
                repo: string,
                number: number,
                body: string
            ) =>
                invokeCommand<GithubIssueComment>(
                    "github_create_issue_comment",
                    { args: { owner, repo, number, body } }
                ),
            updateIssue: (
                owner: string,
                repo: string,
                number: number,
                body: UpdateIssueBody
            ) =>
                invokeCommand<GithubIssueDetail>("github_update_issue", {
                    args: { owner, repo, number, body },
                }),
            updateIssueComment: (
                owner: string,
                repo: string,
                commentId: number,
                body: string
            ) =>
                invokeCommand<GithubIssueComment>(
                    "github_update_issue_comment",
                    { args: { owner, repo, commentId, body } }
                ),
            deleteIssueComment: (
                owner: string,
                repo: string,
                commentId: number
            ) =>
                invokeCommand<void>("github_delete_issue_comment", {
                    args: { owner, repo, commentId },
                }),
            repoPermissions: (owner: string, repo: string) =>
                invokeCommand<GithubRepoPermissions>(
                    "github_repo_permissions",
                    { args: { owner, repo } }
                ),
            createIssue: (
                owner: string,
                repo: string,
                title: string,
                body?: string,
                labels?: string[]
            ) =>
                invokeCommand<GithubIssueDetail>("github_create_issue", {
                    args: { owner, repo, title, body, labels },
                }),
            publishRepository: (
                repoId: number,
                options: {
                    owner?: string;
                    name: string;
                    description?: string;
                    private?: boolean;
                },
                expectedGeneration?: number
            ) =>
                invokeCommand<PublishResult>("github_publish_repository", {
                    args: { repoId, ...options, expectedGeneration },
                }),
        },

        settings: {
            load: () => invokeCommand<SettingsSnapshot>("settings_load"),
            set: (key: string, value: SettingValue) =>
                invokeCommand<ValuesSnapshot>("settings_set", {
                    args: { key, value },
                }),
        },

        editor: {
            /** Opens `path` (optionally `relativePath` joined onto it) in
             * the editor configured by the `editorCommand` setting. */
            openInEditor: (path: string, relativePath?: string) =>
                invokeCommand<void>("open_in_editor", {
                    args: { path, relativePath },
                }),
        },

        fileManager: {
            /** Reveals `path` (optionally `relativePath` joined onto it) in
             * the OS file manager: directories open, files are selected in
             * their parent. */
            reveal: (path: string, relativePath?: string) =>
                invokeCommand<void>("reveal_in_file_manager", {
                    args: { path, relativePath },
                }),
        },

        icons: {
            resolve: (remoteUrl: string) =>
                invokeCommand<CachedIcon>("icon_resolve", {
                    args: { remoteUrl },
                }),
            refresh: (remoteUrl: string) =>
                invokeCommand<CachedIcon>("icon_refresh", {
                    args: { remoteUrl },
                }),
            byRepo: (repoId: number) =>
                invokeCommand<CachedIcon | null>("git_repo_icon", {
                    args: { repoId },
                }),
            byPath: (path: string) =>
                invokeCommand<CachedIcon | null>("git_repo_icon_by_path", {
                    args: { path },
                }),
        },

        session: {
            load: () => invokeCommand<SessionDocument | null>("session_load"),
            save: (doc: SessionDocument) =>
                invokeCommand<void>("session_save", { args: { doc } }),
        },
    };
}

type RawClient = ReturnType<typeof createRawBackendClient>;

/** `(...args) => Promise<T>` becomes `(...args) => Promise<Result<T>>`. */
type Resultified<TFn> = TFn extends (
    ...args: infer Args
) => Promise<infer Value>
    ? (...args: Args) => Promise<Result<Value>>
    : never;

export type BackendClient = {
    [TGroup in keyof RawClient]: {
        [TMethod in keyof RawClient[TGroup]]: Resultified<
            RawClient[TGroup][TMethod]
        >;
    };
};

function resultifyGroup<TGroup extends Record<string, unknown>>(
    group: TGroup
): { [TKey in keyof TGroup]: Resultified<TGroup[TKey]> } {
    const out = {} as { [TKey in keyof TGroup]: Resultified<TGroup[TKey]> };
    for (const key of Object.keys(group) as Array<keyof TGroup & string>) {
        const method = group[key] as (...args: never[]) => Promise<unknown>;
        out[key] = ((...args: never[]) =>
            asResult(() => method(...args))) as Resultified<TGroup[typeof key]>;
    }
    return out;
}

/**
 * The public client: identical signatures to the raw one, but every call
 * resolves to a Result instead of rejecting.
 */
export function createBackendClient(): BackendClient {
    const raw = createRawBackendClient();
    return {
        repositories: resultifyGroup(raw.repositories),
        changes: resultifyGroup(raw.changes),
        hooks: resultifyGroup(raw.hooks),
        diff: resultifyGroup(raw.diff),
        graph: resultifyGroup(raw.graph),
        history: resultifyGroup(raw.history),
        refs: resultifyGroup(raw.refs),
        mutations: resultifyGroup(raw.mutations),
        worktrees: resultifyGroup(raw.worktrees),
        workflows: resultifyGroup(raw.workflows),
        remoteInfo: resultifyGroup(raw.remoteInfo),
        remotes: resultifyGroup(raw.remotes),
        operations: resultifyGroup(raw.operations),
        highlight: resultifyGroup(raw.highlight),
        github: resultifyGroup(raw.github),
        settings: resultifyGroup(raw.settings),
        editor: resultifyGroup(raw.editor),
        fileManager: resultifyGroup(raw.fileManager),
        icons: resultifyGroup(raw.icons),
        session: resultifyGroup(raw.session),
    };
}

export const REPOSITORY_INVALIDATED_EVENT = "repository-invalidated";

export interface InvalidationListenerDisposer {
    dispose: () => void;
}

export async function listenRepositoryInvalidations(
    onInvalidation: (invalidation: RepositoryInvalidation) => void
): Promise<InvalidationListenerDisposer> {
    const { listen } = await import("@tauri-apps/api/event");
    const unlisten = await listen<RepositoryInvalidation>(
        REPOSITORY_INVALIDATED_EVENT,
        (event) => onInvalidation(event.payload)
    );
    return { dispose: unlisten };
}
