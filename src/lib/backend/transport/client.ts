import { Channel } from "@tauri-apps/api/core";

import type {
    CloneEvent,
    CommitHookEvent,
    CreatePullRequestBody,
    CreateRepositoryRequest,
    CheckoutMode,
    CredentialRequest,
    DiffEvent,
    DiffRequest,
    GraphEvent,
    GraphQuery,
    HistoryChartQuery,
    HistoryPageQuery,
    HookOutputChunk,
    MergePullRequestBody,
    RepositoryInvalidation,
    ResetKind,
    ResolveSide,
    SessionDocument,
    SettingValue,
    StatusOptions,
    UpdateIssueBody,
    UpdatePullRequestBody,
} from "../protocol";
import { commands } from "./commands.generated";
import { asResult } from "./result";
import type { Result } from "./result";

/**
 * The ergonomic backend client. Every method returns a `Result`: the client
 * NEVER rejects, so callers are forced by the compiler to narrow on `ok`
 * before using a value.
 */
function createRawBackendClient() {
    return {
        repositories: {
            open: (path: string) =>
                commands.git_open_repository({ request: { path } }),
            close: (repoId: number) =>
                commands.git_close_repository({ repoId }),
            remove: (path: string, moveToTrash: boolean) =>
                commands.git_remove_repository({ path, moveToTrash }),
            snapshot: (repoId: number) =>
                commands.git_repository_snapshot({ repoId }),
            readme: (repoId: number) => commands.git_readme({ repoId }),
            worktreeImage: (repoId: number, path: string) =>
                commands.git_read_worktree_image({ repoId, path }),
            create: (request: CreateRepositoryRequest) =>
                commands.git_init_repository({ request }),
            gitignoreTemplates: () => commands.git_list_gitignore_templates(),
            licenses: () => commands.git_list_licenses(),
        },

        changes: {
            status: (repoId: number, options?: StatusOptions) =>
                commands.git_status({ repoId, options }),
            stagePaths: (
                repoId: number,
                paths: string[],
                all: boolean | undefined,
                expectedGeneration?: number
            ) =>
                commands.git_stage_paths({
                    repoId,
                    paths,
                    all,
                    expectedGeneration,
                }),
            unstagePaths: (
                repoId: number,
                paths: string[],
                expectedGeneration?: number
            ) =>
                commands.git_unstage_paths({
                    repoId,
                    paths,
                    expectedGeneration,
                }),
            discardChanges: (
                repoId: number,
                paths: string[],
                all: boolean | undefined,
                expectedGeneration?: number
            ) =>
                commands.git_discard_changes({
                    repoId,
                    paths,
                    all,
                    expectedGeneration,
                }),
        },

        hooks: {
            list: (repoId: number) =>
                commands.git_list_commit_hooks({ repoId }),
            /**
             * Runs one hook, handing `onLine` each output line as the process
             * writes it. Resolves once every line has been delivered, with a
             * result whose `lines` repeat the stream.
             */
            runStreamed: (
                repoId: number,
                hook: string,
                onLine: (chunk: HookOutputChunk) => void
            ) => {
                const channel = new Channel<HookOutputChunk>();
                channel.onmessage = onLine;
                return commands.git_run_commit_hook_streamed({
                    repoId,
                    hook,
                    onEvent: channel,
                });
            },
            read: (repoId: number, hook: string) =>
                commands.git_read_commit_hook({ repoId, hook }),
            write: (repoId: number, hook: string, content: string) =>
                commands.git_write_commit_hook({ repoId, hook, content }),
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
                return commands.git_open_diff({
                    repoId,
                    request,
                    onEvent: channel,
                });
            },
            readImage: (operationId: number, sectionId: number) =>
                commands.git_read_diff_image({ operationId, sectionId }),
            readRange: (
                operationId: number,
                startRow: number,
                maxRows: number
            ) =>
                commands.git_read_diff_range({
                    operationId,
                    startRow,
                    maxRows,
                }),
            readContext: (
                operationId: number,
                sectionId: number,
                oldStart: number,
                newStart: number,
                count: number
            ) =>
                commands.git_read_diff_context({
                    operationId,
                    sectionId,
                    oldStart,
                    newStart,
                    count,
                }),
            cancel: (operationId: number) =>
                commands.git_cancel_operation({ operationId }),
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
                return commands.git_open_graph({
                    repoId,
                    query,
                    onEvent: channel,
                });
            },
            readImage: (operationId: number, sectionId: number) =>
                commands.git_read_diff_image({ operationId, sectionId }),
            readRange: (
                operationId: number,
                startRow: number,
                maxRows: number
            ) =>
                commands.git_read_graph_range({
                    operationId,
                    startRow,
                    maxRows,
                }),
            cancel: (operationId: number) =>
                commands.git_cancel_operation({ operationId }),
        },

        history: {
            page: (repoId: number, query: HistoryPageQuery = {}) =>
                commands.git_history_page({ repoId, query }),
            chart: (repoId: number, query: HistoryChartQuery = {}) =>
                commands.git_history_chart({ repoId, query }),
            commitDetail: (
                repoId: number,
                revision: string,
                detectRenames?: boolean
            ) =>
                commands.git_commit_detail({ repoId, revision, detectRenames }),
            fileAtRevision: (repoId: number, revision: string, path: string) =>
                commands.git_file_at_revision({ repoId, revision, path }),
            blame: (repoId: number, path: string, revision?: string) =>
                commands.git_blame({ repoId, path, revision }),
        },

        refs: {
            listBranchesAndTags: (repoId: number) =>
                commands.git_list_branches_and_tags({ repoId }),
            /** Commits `head` has that `base` lacks, and the reverse. */
            aheadBehind: (repoId: number, head: string, base: string) =>
                commands.git_branch_ahead_behind({ repoId, head, base }),
            createBranch: (
                repoId: number,
                name: string,
                startPoint: string | undefined,
                force: boolean | undefined,
                checkout: boolean | undefined,
                expectedGeneration?: number
            ) =>
                commands.git_create_branch({
                    repoId,
                    name,
                    startPoint,
                    force,
                    checkout,
                    expectedGeneration,
                }),
            deleteBranch: (
                repoId: number,
                name: string,
                force: boolean | undefined,
                expectedGeneration?: number
            ) =>
                commands.git_delete_branch({
                    repoId,
                    name,
                    force,
                    expectedGeneration,
                }),
            renameBranch: (
                repoId: number,
                oldName: string,
                newName: string,
                force: boolean | undefined,
                expectedGeneration?: number
            ) =>
                commands.git_rename_branch({
                    repoId,
                    oldName,
                    newName,
                    force,
                    expectedGeneration,
                }),
            createTag: (
                repoId: number,
                name: string,
                target: string | undefined,
                message: string | undefined,
                force: boolean | undefined,
                expectedGeneration?: number
            ) =>
                commands.git_create_tag({
                    repoId,
                    name,
                    target,
                    message,
                    force,
                    expectedGeneration,
                }),
            deleteTag: (
                repoId: number,
                name: string,
                expectedGeneration?: number
            ) => commands.git_delete_tag({ repoId, name, expectedGeneration }),
        },

        mutations: {
            /**
             * Commits, reporting each hook the pipeline runs through
             * `options.onHookEvent`: it started, wrote a line, and settled.
             * Mirrors the streamed hook command, minus the ability to choose
             * which hook runs.
             */
            commit: (
                repoId: number,
                message: string,
                options: {
                    stageAll?: boolean;
                    allowEmpty?: boolean;
                    authorName?: string;
                    authorEmail?: string;
                    /** Called for every hook the commit runs, in the order
                     * they run. */
                    onHookEvent?: (event: CommitHookEvent) => void;
                } = {},
                expectedGeneration?: number
            ) => {
                // The sink stays on this side of the wire; the rest of the
                // options are the command's arguments.
                const { onHookEvent, ...wire } = options;
                const channel = new Channel<CommitHookEvent>();
                channel.onmessage = onHookEvent ?? ignoreHookEvent;
                return commands.git_commit({
                    repoId,
                    message,
                    ...wire,
                    expectedGeneration,
                    onEvent: channel,
                });
            },
            amend: (
                repoId: number,
                message?: string,
                expectedGeneration?: number
            ) =>
                commands.git_amend_commit({
                    repoId,
                    message,
                    expectedGeneration,
                }),
            checkout: (
                repoId: number,
                target: string,
                options: {
                    force?: boolean;
                    mode?: CheckoutMode;
                    paths?: string[];
                } = {},
                expectedGeneration?: number
            ) =>
                commands.git_checkout({
                    repoId,
                    target,
                    ...options,
                    expectedGeneration,
                }),
            reset: (
                repoId: number,
                kind: ResetKind,
                target: string,
                expectedGeneration?: number
            ) =>
                commands.git_reset({
                    repoId,
                    kind,
                    target,
                    expectedGeneration,
                }),
        },

        worktrees: {
            list: (repoId: number) => commands.git_list_worktrees({ repoId }),
            create: (
                repoId: number,
                name: string,
                path: string | undefined,
                startPoint: string | undefined,
                expectedGeneration?: number
            ) =>
                commands.git_create_worktree({
                    repoId,
                    name,
                    path,
                    startPoint,
                    expectedGeneration,
                }),
            remove: (
                repoId: number,
                name: string,
                force: boolean | undefined,
                expectedGeneration?: number
            ) =>
                commands.git_remove_worktree({
                    repoId,
                    name,
                    force,
                    expectedGeneration,
                }),
            lock: (
                repoId: number,
                name: string,
                reason: string | undefined,
                expectedGeneration?: number
            ) =>
                commands.git_lock_worktree({
                    repoId,
                    name,
                    reason,
                    expectedGeneration,
                }),
            unlock: (
                repoId: number,
                name: string,
                expectedGeneration?: number
            ) =>
                commands.git_unlock_worktree({
                    repoId,
                    name,
                    expectedGeneration,
                }),
        },

        submodules: {
            list: (repoId: number) => commands.git_list_submodules({ repoId }),
            add: (
                repoId: number,
                url: string,
                path: string,
                branch?: string,
                expectedGeneration?: number
            ) =>
                commands.git_add_submodule({
                    repoId,
                    url,
                    path,
                    branch,
                    expectedGeneration,
                }),
            update: (
                repoId: number,
                options: {
                    names?: string[];
                    recursive?: boolean;
                    init?: boolean;
                } = {},
                expectedGeneration?: number
            ) =>
                commands.git_update_submodules({
                    repoId,
                    ...options,
                    expectedGeneration,
                }),
        },

        lfs: {
            status: (repoId: number) => commands.git_lfs_status({ repoId }),
            smudge: (repoId: number, expectedGeneration?: number) =>
                commands.git_lfs_smudge({ repoId, expectedGeneration }),
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
                commands.git_merge({
                    repoId,
                    target,
                    ...options,
                    expectedGeneration,
                }),
            mergeContinue: (
                repoId: number,
                message?: string,
                expectedGeneration?: number
            ) =>
                commands.git_merge_continue({
                    repoId,
                    message,
                    expectedGeneration,
                }),
            mergeAbort: (repoId: number, expectedGeneration?: number) =>
                commands.git_merge_abort({ repoId, expectedGeneration }),
            startRebase: (
                repoId: number,
                upstream: string,
                onto?: string,
                expectedGeneration?: number
            ) =>
                commands.git_start_rebase({
                    repoId,
                    upstream,
                    onto,
                    expectedGeneration,
                }),
            continueRebase: (repoId: number, expectedGeneration?: number) =>
                commands.git_continue_rebase({ repoId, expectedGeneration }),
            abortRebase: (repoId: number, expectedGeneration?: number) =>
                commands.git_abort_rebase({ repoId, expectedGeneration }),
            operationState: (repoId: number) =>
                commands.git_operation_state({ repoId }),
            resolveConflict: (
                repoId: number,
                path: string,
                side: ResolveSide,
                expectedGeneration?: number
            ) =>
                commands.git_resolve_conflict({
                    repoId,
                    path,
                    side,
                    expectedGeneration,
                }),
            conflictFile: (repoId: number, path: string, stage: number) =>
                commands.git_conflict_file({ repoId, path, stage }),
            cherryPick: (
                repoId: number,
                target: string,
                expectedGeneration?: number
            ) =>
                commands.git_cherry_pick({
                    repoId,
                    target,
                    expectedGeneration,
                }),
            revert: (
                repoId: number,
                target: string,
                parentIndex?: number,
                expectedGeneration?: number
            ) =>
                commands.git_revert({
                    repoId,
                    target,
                    parentIndex,
                    expectedGeneration,
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
                commands.git_stash_push({
                    repoId,
                    ...options,
                    expectedGeneration,
                }),
            stashPop: (
                repoId: number,
                index: number,
                action?: "pop" | "apply" | "drop",
                expectedGeneration?: number
            ) =>
                commands.git_stash_pop({
                    repoId,
                    index,
                    action,
                    expectedGeneration,
                }),
            stashList: (repoId: number) => commands.git_stash_list({ repoId }),
        },

        remoteInfo: {
            info: (repoId: number) => commands.git_remote_repo_info({ repoId }),
            byPath: (path: string) =>
                commands.git_remote_repo_info_by_path({ path }),
        },

        remotes: {
            list: (repoId: number) => commands.git_list_remotes({ repoId }),
            listByPath: (path: string) =>
                commands.git_list_remotes_by_path({ path }),
            add: (
                repoId: number,
                name: string,
                url: string,
                expectedGeneration?: number
            ) =>
                commands.git_add_remote({
                    repoId,
                    name,
                    url,
                    expectedGeneration,
                }),
            remove: (
                repoId: number,
                name: string,
                expectedGeneration?: number
            ) =>
                commands.git_remove_remote({
                    repoId,
                    name,
                    expectedGeneration,
                }),
            setUrl: (
                repoId: number,
                name: string,
                url: string,
                expectedGeneration?: number
            ) =>
                commands.git_set_remote_url({
                    repoId,
                    name,
                    url,
                    expectedGeneration,
                }),
            fetch: (
                repoId: number,
                options: {
                    remote?: string;
                    prune?: boolean;
                    credential?: CredentialRequest;
                } = {}
            ) => commands.git_fetch({ repoId, ...options }),
            push: (
                repoId: number,
                options: {
                    remote?: string;
                    refspecs?: string[];
                    force?: boolean;
                    setUpstream?: boolean;
                    credential?: CredentialRequest;
                } = {}
            ) => commands.git_push({ repoId, ...options }),
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
            ) => commands.git_pull({ repoId, ...options, expectedGeneration }),
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
                return commands.git_clone({ request, onEvent: channel });
            },
        },

        operations: {
            cancel: (operationId: number) =>
                commands.git_cancel_operation({ operationId }),
        },

        highlight: {
            code: (language: string, text: string) =>
                commands.highlight_code({ language, text }),
        },

        forge: {
            providers: () => commands.forge_providers(),
            connections: () => commands.forge_connections(),
            account: (provider: string) => commands.forge_account({ provider }),
            beginSignIn: (provider: string, host: string) =>
                commands.forge_begin_sign_in({ provider, host }),
            completeSignIn: () => commands.forge_complete_sign_in(),
            cancelSignIn: () => commands.forge_cancel_sign_in(),
            signOut: (provider: string, host: string) =>
                commands.forge_sign_out({ provider, host }),
            listOrgs: (provider: string) =>
                commands.forge_list_orgs({ provider }),
            listNotifications: (provider: string, page?: number) =>
                commands.forge_list_notifications({ provider, page }),
            markNotificationRead: (provider: string, threadId: string) =>
                commands.forge_mark_notification_read({ provider, threadId }),
            markAllNotificationsRead: (provider: string) =>
                commands.forge_mark_all_notifications_read({ provider }),
            resolveSubjectUrl: (provider: string, subjectUrl: string) =>
                commands.forge_resolve_subject_url({ provider, subjectUrl }),
            listIssues: (
                provider: string,
                owner: string,
                repo: string,
                issueState?: string,
                labels?: string[],
                page?: number
            ) =>
                commands.forge_list_issues({
                    provider,
                    owner,
                    repo,
                    issueState,
                    labels,
                    page,
                }),
            listPullRequests: (
                provider: string,
                owner: string,
                repo: string,
                pullState?: string,
                labels?: string[],
                page?: number
            ) =>
                commands.forge_list_pull_requests({
                    provider,
                    owner,
                    repo,
                    pullState,
                    labels,
                    page,
                }),
            getPull: (
                provider: string,
                owner: string,
                repo: string,
                number: number
            ) => commands.forge_get_pull({ provider, owner, repo, number }),
            listPullReviews: (
                provider: string,
                owner: string,
                repo: string,
                number: number
            ) =>
                commands.forge_list_pull_reviews({
                    provider,
                    owner,
                    repo,
                    number,
                }),
            listPullCommits: (
                provider: string,
                owner: string,
                repo: string,
                number: number
            ) =>
                commands.forge_list_pull_commits({
                    provider,
                    owner,
                    repo,
                    number,
                }),
            listPullReviewComments: (
                provider: string,
                owner: string,
                repo: string,
                number: number
            ) =>
                commands.forge_list_pull_review_comments({
                    provider,
                    owner,
                    repo,
                    number,
                }),
            createPull: (
                provider: string,
                owner: string,
                repo: string,
                body: CreatePullRequestBody
            ) => commands.forge_create_pull({ provider, owner, repo, body }),
            listLabels: (provider: string, owner: string, repo: string) =>
                commands.forge_list_labels({ provider, owner, repo }),
            updatePull: (
                provider: string,
                owner: string,
                repo: string,
                number: number,
                body: UpdatePullRequestBody
            ) =>
                commands.forge_update_pull({
                    provider,
                    owner,
                    repo,
                    number,
                    body,
                }),
            mergePull: (
                provider: string,
                owner: string,
                repo: string,
                number: number,
                body: MergePullRequestBody
            ) =>
                commands.forge_merge_pull({
                    provider,
                    owner,
                    repo,
                    number,
                    body,
                }),
            getCheckRun: (
                provider: string,
                owner: string,
                repo: string,
                checkRunId: number
            ) =>
                commands.forge_get_check_run({
                    provider,
                    owner,
                    repo,
                    checkRunId,
                }),
            getCheckRunLog: (
                provider: string,
                owner: string,
                repo: string,
                checkRunId: number
            ) =>
                commands.forge_get_check_run_log({
                    provider,
                    owner,
                    repo,
                    checkRunId,
                }),
            listCheckRuns: (
                provider: string,
                owner: string,
                repo: string,
                sha: string
            ) => commands.forge_list_check_runs({ provider, owner, repo, sha }),
            listWorkflowRuns: (
                provider: string,
                owner: string,
                repo: string,
                sha: string
            ) =>
                commands.forge_list_workflow_runs({
                    provider,
                    owner,
                    repo,
                    sha,
                }),
            listCommitStatuses: (
                provider: string,
                owner: string,
                repo: string,
                sha: string
            ) =>
                commands.forge_list_commit_statuses({
                    provider,
                    owner,
                    repo,
                    sha,
                }),
            searchIssues: (provider: string, page?: number) =>
                commands.forge_search_issues({ provider, page }),
            searchPullRequests: (provider: string, page?: number) =>
                commands.forge_search_pull_requests({ provider, page }),
            getIssue: (
                provider: string,
                owner: string,
                repo: string,
                number: number
            ) => commands.forge_get_issue({ provider, owner, repo, number }),
            listIssueComments: (
                provider: string,
                owner: string,
                repo: string,
                number: number
            ) =>
                commands.forge_list_issue_comments({
                    provider,
                    owner,
                    repo,
                    number,
                }),
            listIssueEvents: (
                provider: string,
                owner: string,
                repo: string,
                number: number
            ) =>
                commands.forge_list_issue_events({
                    provider,
                    owner,
                    repo,
                    number,
                }),
            createIssueComment: (
                provider: string,
                owner: string,
                repo: string,
                number: number,
                body: string
            ) =>
                commands.forge_create_issue_comment({
                    provider,
                    owner,
                    repo,
                    number,
                    body,
                }),
            updateIssue: (
                provider: string,
                owner: string,
                repo: string,
                number: number,
                body: UpdateIssueBody
            ) =>
                commands.forge_update_issue({
                    provider,
                    owner,
                    repo,
                    number,
                    body,
                }),
            updateIssueComment: (
                provider: string,
                owner: string,
                repo: string,
                commentId: number,
                body: string
            ) =>
                commands.forge_update_issue_comment({
                    provider,
                    owner,
                    repo,
                    commentId,
                    body,
                }),
            deleteIssueComment: (
                provider: string,
                owner: string,
                repo: string,
                commentId: number
            ) =>
                commands.forge_delete_issue_comment({
                    provider,
                    owner,
                    repo,
                    commentId,
                }),
            repoPermissions: (provider: string, owner: string, repo: string) =>
                commands.forge_repo_permissions({ provider, owner, repo }),
            createIssue: (
                provider: string,
                owner: string,
                repo: string,
                title: string,
                body?: string,
                labels?: string[]
            ) =>
                commands.forge_create_issue({
                    provider,
                    owner,
                    repo,
                    title,
                    body,
                    labels,
                }),
            publishRepository: (
                repoId: number,
                options: {
                    provider?: string;
                    owner?: string;
                    name: string;
                    description?: string;
                    private?: boolean;
                },
                expectedGeneration?: number
            ) =>
                commands.forge_publish_repository({
                    repoId,
                    ...options,
                    expectedGeneration,
                }),
        },

        settings: {
            load: () => commands.settings_load(),
            set: (key: string, value: SettingValue) =>
                commands.settings_set({ key, value }),
        },

        fonts: {
            list: () => commands.fonts_list(),
        },

        editor: {
            /** Opens `path` (optionally `relativePath` joined onto it) in
             * the editor configured by the `editorCommand` setting. */
            openInEditor: (path: string, relativePath?: string) =>
                commands.open_in_editor({ path, relativePath }),
        },

        fileManager: {
            /** Reveals `path` (optionally `relativePath` joined onto it) in
             * the OS file manager: directories open, files are selected in
             * their parent. */
            reveal: (path: string, relativePath?: string) =>
                commands.reveal_in_file_manager({ path, relativePath }),
        },

        terminal: {
            /** Opens a terminal with `path` as its working directory, using
             * the system default emulator or the `terminalCommand` override
             * when one is set. */
            open: (path: string) => commands.open_in_terminal({ path }),
        },

        icons: {
            resolve: (remoteUrl: string) =>
                commands.icon_resolve({ remoteUrl }),
            refresh: (remoteUrl: string) =>
                commands.icon_refresh({ remoteUrl }),
            byRepo: (repoId: number) => commands.git_repo_icon({ repoId }),
            byPath: (path: string) => commands.git_repo_icon_by_path({ path }),
        },

        session: {
            load: () => commands.session_load(),
            save: (doc: SessionDocument) => commands.session_save({ doc }),
            repositories: (paths: string[]) =>
                commands.session_repositories({ paths }),
        },
    };
}

type RawClient = ReturnType<typeof createRawBackendClient>;

/** Stands in for a caller with nowhere to put hook progress. The channel
 * still has to be handed to the backend, which reads it as a required
 * argument. */
function ignoreHookEvent(_event: CommitHookEvent): void {}

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
        submodules: resultifyGroup(raw.submodules),
        lfs: resultifyGroup(raw.lfs),
        workflows: resultifyGroup(raw.workflows),
        remoteInfo: resultifyGroup(raw.remoteInfo),
        remotes: resultifyGroup(raw.remotes),
        operations: resultifyGroup(raw.operations),
        highlight: resultifyGroup(raw.highlight),
        forge: resultifyGroup(raw.forge),
        settings: resultifyGroup(raw.settings),
        fonts: resultifyGroup(raw.fonts),
        editor: resultifyGroup(raw.editor),
        fileManager: resultifyGroup(raw.fileManager),
        terminal: resultifyGroup(raw.terminal),
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
