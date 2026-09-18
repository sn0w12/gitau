import { toastManager } from "@/components/ui/toast";
import {
    useFetchMutation,
    usePullMutation,
    usePushMutation,
} from "@/lib/backend/mutations/repository-mutations";
import { summarizeWorkflowOutcome } from "@/lib/backend/mutations/workflow-outcome";
import type { PushOutcome, WorkflowOutcome } from "@/lib/backend/protocol";
import { primaryRemoteName } from "@/lib/repositories/fetch-action";
import { toastError } from "@/lib/toast-error";
import { markRepositoryFetched } from "@/stores/fetch-store";

import { useRemotes } from "./use-repository-queries";

const MAX_LISTED_CONFLICT_PATHS = 5;

export interface SyncOptions {
    setUpstream?: boolean;
    force?: boolean;
}

/**
 * Fetch, pull, and push flows shared by the sync button, the repo
 * shortcuts, and the command palette. Errors toast; pull and push
 * outcomes are summarized.
 */
export function useSyncActions(repoId: number, repoPath: string) {
    const fetchMutation = useFetchMutation(repoId);
    const pullMutation = usePullMutation(repoId);
    const pushMutation = usePushMutation(repoId);
    const remotes = useRemotes(repoId);
    const remoteName = primaryRemoteName(remotes.data ?? []);

    const markFetched = () => {
        if (repoPath) markRepositoryFetched(repoPath);
    };

    const fetch = async () => {
        try {
            // Prune stale remote-tracking refs, matching GitHub Desktop's
            // `git fetch --prune`.
            await fetchMutation.mutateAsync({
                remote: remoteName,
                prune: true,
            });
            markFetched();
        } catch (error) {
            toastError("Fetch failed", error);
        }
    };

    const pull = async () => {
        try {
            const outcome = await pullMutation.mutateAsync({
                remote: remoteName,
            });
            markFetched();
            reportPullOutcome(outcome, remoteName);
        } catch (error) {
            toastError("Pull failed", error);
        }
    };

    const push = async (options: SyncOptions = {}) => {
        try {
            const outcomes = await pushMutation.mutateAsync({
                remote: remoteName,
                ...options,
            });
            markFetched();
            reportPushOutcome(
                outcomes,
                remoteName,
                options.setUpstream === true
            );
        } catch (error) {
            toastError("Push failed", error);
        }
    };

    return {
        fetch,
        pull,
        push,
        remoteName,
        busy:
            fetchMutation.isPending ||
            pullMutation.isPending ||
            pushMutation.isPending,
    };
}

function reportPullOutcome(
    outcome: WorkflowOutcome,
    remoteName: string | undefined
): void {
    const summary = summarizeWorkflowOutcome(outcome);
    if (summary.conflictPaths.length > 0) {
        const shown = summary.conflictPaths.slice(0, MAX_LISTED_CONFLICT_PATHS);
        const rest =
            summary.conflictPaths.length - shown.length > 0
                ? ` (+${summary.conflictPaths.length - shown.length} more)`
                : "";
        toastManager.add({
            title: "Pull stopped on conflicts",
            description: `${shown.join("\n")}${rest}`,
            type: "warning",
        });
        return;
    }
    toastManager.add({
        title: `Pulled ${remoteName ? `from ${remoteName}` : ""}`.trim(),
        description: summary.message,
        type: "success",
    });
}

function reportPushOutcome(
    outcomes: PushOutcome[],
    remoteName: string | undefined,
    publishing: boolean
): void {
    const rejected = outcomes.filter((outcome) => !outcome.accepted);
    if (rejected.length > 0) {
        toastManager.add({
            title: "Push rejected",
            description: rejected
                .map(
                    (outcome) =>
                        `${outcome.reference}: ${outcome.reason ?? "rejected"}`
                )
                .join("\n"),
            type: "error",
        });
        return;
    }
    toastManager.add({
        title: publishing
            ? "Branch published"
            : `Pushed ${remoteName ? `to ${remoteName}` : ""}`.trim(),
        type: "success",
    });
}
