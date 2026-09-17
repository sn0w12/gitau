import { useMutation } from "@tanstack/react-query";

import { useAppServices } from "@/contexts/services-context";
import {
    generationOf,
    invalidateRepository,
} from "@/lib/backend/mutations/invalidation";
import type { InvalidationScope } from "@/lib/backend/mutations/invalidation";
import type {
    BranchInfo,
    CommitExecution,
    CommitSummary,
    PublishResult,
    PushOutcome,
    ResetKind,
    ResolveSide,
    StashEntry,
    TagInfo,
    WorkflowOutcome,
    WorktreeInfo,
} from "@/lib/backend/protocol";
import { snapshotQuery } from "@/lib/backend/queries/repository-queries";
import type { BackendClient } from "@/lib/backend/transport/client";
import { expectOk } from "@/lib/backend/transport/result";

/**
 * Write callbacks unwrap the client Result via `.then(expectOk)` so the
 * mutation's failure channel is react-query's rejection, surfaced through
 * `mutation.error`.
 */

interface RepoWriteOptions<TInput> {
    scopes:
        | readonly InvalidationScope[]
        | ((input: TInput) => readonly InvalidationScope[]);
}

type CommitInput = {
    message: string;
    stageAll?: boolean;
    allowEmpty?: boolean;
    authorName?: string;
    authorEmail?: string;
};

type CommitResult = CommitExecution;

type PushOptions = Parameters<BackendClient["remotes"]["push"]>[1];
type PushResult = PushOutcome[];

/**
 * Shared plumbing for repository writes: reads the authoritative generation
 * from the snapshot query, serializes writes per repo via a mutation scope,
 * and refreshes affected queries after success.
 *
 * Git mutations are never applied optimistically to status/history; the
 * backend is the source of truth and conflicts are common.
 */
export function useRepoWrite<TInput, TResult>(
    repoId: number,
    write: (
        backend: BackendClient,
        input: TInput,
        expectedGeneration?: number
    ) => Promise<TResult>,
    options: RepoWriteOptions<TInput>
) {
    const { backend, queryClient } = useAppServices();

    return useMutation({
        scope: { id: `repo:${repoId}` },
        mutationFn: async (input: TInput) => {
            let expectedGeneration = generationOf(queryClient, repoId);
            // Write callbacks already unwrap via `.then(expectOk)`.
            const attempt = () => write(backend, input, expectedGeneration);
            try {
                return await attempt();
            } catch (error) {
                // The backend rejects writes whose generation is stale:
                // a watcher bump (build output, IDE save) landed after our
                // cached snapshot. The rejected write never ran, so refresh
                // the snapshot and retry once with the authoritative
                // generation; without this the user's own stage is refused
                // even though nothing went wrong.
                if (!isStaleSnapshotError(error)) throw error;
                await queryClient.fetchQuery(
                    snapshotQuery({ backend }, repoId)
                );
                expectedGeneration = generationOf(queryClient, repoId);
                return attempt();
            }
        },
        onSuccess: (_data, variables) =>
            invalidateRepository(
                queryClient,
                repoId,
                typeof options.scopes === "function"
                    ? options.scopes(variables)
                    : options.scopes
            ),
    });
}

function isStaleSnapshotError(error: unknown): boolean {
    if (typeof error !== "object" || error === null) return false;
    const candidate = error as { isStaleSnapshot?: boolean; code?: string };
    return (
        candidate.isStaleSnapshot === true || candidate.code === "staleSnapshot"
    );
}

export function useStageMutation(repoId: number) {
    return useRepoWrite<{ paths: string[]; all?: boolean }, void>(
        repoId,
        (backend, input, expectedGeneration) =>
            backend.changes
                .stagePaths(repoId, input.paths, input.all, expectedGeneration)
                .then(expectOk),
        { scopes: ["status", "operation"] }
    );
}

export function useUnstageMutation(repoId: number) {
    return useRepoWrite<{ paths: string[] }, void>(
        repoId,
        (backend, input, expectedGeneration) =>
            backend.changes
                .unstagePaths(repoId, input.paths, expectedGeneration)
                .then(expectOk),
        { scopes: ["status", "operation"] }
    );
}

export function useDiscardMutation(repoId: number) {
    return useRepoWrite<{ paths: string[]; all?: boolean }, void>(
        repoId,
        (backend, input, expectedGeneration) =>
            backend.changes
                .discardChanges(
                    repoId,
                    input.paths,
                    input.all,
                    expectedGeneration
                )
                .then(expectOk),
        { scopes: ["status", "files", "operation"] }
    );
}

export function useCommitMutation(repoId: number) {
    return useRepoWrite<CommitInput, CommitResult>(
        repoId,
        (backend, input, expectedGeneration) =>
            backend.mutations
                .commit(repoId, input.message, input, expectedGeneration)
                .then(expectOk),
        { scopes: ["status", "history", "refs", "operation"] }
    );
}

export function useFetchMutation(repoId: number) {
    const { backend, queryClient } = useAppServices();
    return useMutation({
        scope: { id: `repo:${repoId}` },
        mutationFn: async (
            options?: Parameters<BackendClient["remotes"]["fetch"]>[1]
        ) => expectOk(await backend.remotes.fetch(repoId, options)),
        onSuccess: () =>
            invalidateRepository(queryClient, repoId, [
                "refs",
                "history",
                "remotes",
            ]),
    });
}

export function usePushMutation(repoId: number) {
    return useRepoWrite<PushOptions | undefined, PushResult>(
        repoId,
        (be, options) => be.remotes.push(repoId, options).then(expectOk),
        { scopes: ["refs", "history"] }
    );
}

type PullOptions = Parameters<BackendClient["remotes"]["pull"]>[1];
type CheckoutTarget = string;
type CreateBranchInput = {
    name: string;
    startPoint?: string;
    /** Defaults to true; branch-from-commit menus create without moving HEAD. */
    checkout?: boolean;
};

export function usePullMutation(repoId: number) {
    return useRepoWrite<PullOptions | undefined, WorkflowOutcome>(
        repoId,
        (be, options, expectedGeneration) =>
            be.remotes.pull(repoId, options, expectedGeneration).then(expectOk),
        { scopes: ["refs", "history", "status", "operation"] }
    );
}

export function useCheckoutMutation(repoId: number) {
    return useRepoWrite<CheckoutTarget, void>(
        repoId,
        (be, target, expectedGeneration) =>
            be.mutations
                .checkout(repoId, target, {}, expectedGeneration)
                .then(expectOk),
        { scopes: ["refs", "history", "status", "files"] }
    );
}

export function useCreateBranchMutation(repoId: number) {
    return useRepoWrite<CreateBranchInput, BranchInfo>(
        repoId,
        (be, input, expectedGeneration) =>
            be.refs
                .createBranch(
                    repoId,
                    input.name,
                    input.startPoint,
                    undefined,
                    input.checkout ?? true,
                    expectedGeneration
                )
                .then(expectOk),
        {
            scopes: (input) =>
                (input.checkout ?? true)
                    ? ["refs", "history", "status", "files"]
                    : ["refs"],
        }
    );
}

export interface PublishToGitHubInput {
    owner?: string;
    name: string;
    description?: string;
    private?: boolean;
}

/**
 * Creates the repository on GitHub, wires up `origin`, and pushes the
 * current branch. The backend injects the stored GitHub token itself.
 */
export function usePublishToGitHubMutation(repoId: number) {
    return useRepoWrite<PublishToGitHubInput, PublishResult>(
        repoId,
        (be, input, expectedGeneration) =>
            be.github
                .publishRepository(repoId, input, expectedGeneration)
                .then(expectOk),
        { scopes: ["remotes", "refs", "history"] }
    );
}

type ResetInput = {
    kind: ResetKind;
    target: string;
};

type RevertInput = {
    target: string;
    /** Merge commits revert against the given parent, 1-based. */
    parentIndex?: number;
};

type CreateTagInput = {
    name: string;
    target?: string;
    /** Non-empty creates an annotated tag; empty is lightweight. */
    message?: string;
    force?: boolean;
};

export function useResetMutation(repoId: number) {
    return useRepoWrite<ResetInput, void>(
        repoId,
        (be, input, expectedGeneration) =>
            be.mutations
                .reset(repoId, input.kind, input.target, expectedGeneration)
                .then(expectOk),
        { scopes: ["status", "history", "refs", "files"] }
    );
}

export function useAmendMutation(repoId: number) {
    return useRepoWrite<{ message: string }, CommitSummary>(
        repoId,
        (be, input, expectedGeneration) =>
            be.mutations
                .amend(repoId, input.message, expectedGeneration)
                .then(expectOk),
        { scopes: ["history", "refs"] }
    );
}

export function useRevertMutation(repoId: number) {
    return useRepoWrite<RevertInput, WorkflowOutcome>(
        repoId,
        (be, input, expectedGeneration) =>
            be.workflows
                .revert(
                    repoId,
                    input.target,
                    input.parentIndex,
                    expectedGeneration
                )
                .then(expectOk),
        { scopes: ["status", "history", "refs", "files"] }
    );
}

export function useCherryPickMutation(repoId: number) {
    return useRepoWrite<{ target: string }, WorkflowOutcome>(
        repoId,
        (be, input, expectedGeneration) =>
            be.workflows
                .cherryPick(repoId, input.target, expectedGeneration)
                .then(expectOk),
        { scopes: ["status", "history", "refs", "files"] }
    );
}

type MergeInput = {
    target: string;
    fastForwardOnly?: boolean;
    noFastForward?: boolean;
    message?: string;
};

export function useMergeMutation(repoId: number) {
    return useRepoWrite<MergeInput, WorkflowOutcome>(
        repoId,
        (be, input, expectedGeneration) =>
            be.workflows
                .merge(repoId, input.target, input, expectedGeneration)
                .then(expectOk),
        {
            scopes: ["status", "history", "refs", "files", "operation"],
        }
    );
}

export function useMergeContinueMutation(repoId: number) {
    return useRepoWrite<{ message?: string }, WorkflowOutcome>(
        repoId,
        (be, input, expectedGeneration) =>
            be.workflows
                .mergeContinue(repoId, input.message, expectedGeneration)
                .then(expectOk),
        {
            scopes: ["status", "history", "refs", "files", "operation"],
        }
    );
}

export function useMergeAbortMutation(repoId: number) {
    return useRepoWrite<void, WorkflowOutcome>(
        repoId,
        (be, _input, expectedGeneration) =>
            be.workflows.mergeAbort(repoId, expectedGeneration).then(expectOk),
        {
            scopes: ["status", "history", "refs", "files", "operation"],
        }
    );
}

export function useResolveConflictMutation(repoId: number) {
    return useRepoWrite<{ path: string; side: ResolveSide }, void>(
        repoId,
        (be, input, expectedGeneration) =>
            be.workflows
                .resolveConflict(
                    repoId,
                    input.path,
                    input.side,
                    expectedGeneration
                )
                .then(expectOk),
        { scopes: ["status", "files", "operation"] }
    );
}

type WorktreeCreateInput = {
    name: string;
    path?: string;
    startPoint?: string;
};

export function useCreateWorktreeMutation(repoId: number) {
    return useRepoWrite<WorktreeCreateInput, WorktreeInfo>(
        repoId,
        (be, input, expectedGeneration) =>
            be.worktrees
                .create(
                    repoId,
                    input.name,
                    input.path,
                    input.startPoint,
                    expectedGeneration
                )
                .then(expectOk),
        // Creating a tree can mint a new local branch; refresh refs too.
        { scopes: ["worktrees", "refs"] }
    );
}

export function useRemoveWorktreeMutation(repoId: number) {
    return useRepoWrite<{ name: string; force?: boolean }, void>(
        repoId,
        (be, input, expectedGeneration) =>
            be.worktrees
                .remove(repoId, input.name, input.force, expectedGeneration)
                .then(expectOk),
        { scopes: ["worktrees", "refs"] }
    );
}

export function useLockWorktreeMutation(repoId: number) {
    return useRepoWrite<{ name: string; reason?: string }, void>(
        repoId,
        (be, input, expectedGeneration) =>
            be.worktrees
                .lock(repoId, input.name, input.reason, expectedGeneration)
                .then(expectOk),
        { scopes: ["worktrees"] }
    );
}

export function useUnlockWorktreeMutation(repoId: number) {
    return useRepoWrite<{ name: string }, void>(
        repoId,
        (be, input, expectedGeneration) =>
            be.worktrees
                .unlock(repoId, input.name, expectedGeneration)
                .then(expectOk),
        { scopes: ["worktrees"] }
    );
}

export function useCreateTagMutation(repoId: number) {
    return useRepoWrite<CreateTagInput, TagInfo>(
        repoId,
        (be, input, expectedGeneration) =>
            be.refs
                .createTag(
                    repoId,
                    input.name,
                    input.target,
                    input.message,
                    input.force,
                    expectedGeneration
                )
                .then(expectOk),
        { scopes: ["refs", "history"] }
    );
}

type StashPushInput = {
    message?: string;
    includeUntracked?: boolean;
    keepIndex?: boolean;
    paths?: string[];
};

type StashPopInput = {
    index: number;
    action?: "pop" | "apply" | "drop";
};

export function useStashPushMutation(repoId: number) {
    return useRepoWrite<StashPushInput, string>(
        repoId,
        (be, input, expectedGeneration) =>
            be.workflows
                .stashPush(repoId, input, expectedGeneration)
                .then(expectOk),
        { scopes: ["status", "files", "stash"] }
    );
}

export function useStashPopMutation(repoId: number) {
    return useRepoWrite<StashPopInput, StashEntry[]>(
        repoId,
        (be, input, expectedGeneration) =>
            be.workflows
                .stashPop(repoId, input.index, input.action, expectedGeneration)
                .then(expectOk),
        { scopes: ["status", "files", "stash"] }
    );
}
