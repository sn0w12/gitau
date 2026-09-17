import type * as React from "react";

import { toastManager } from "@/components/ui/toast";
import { useConfirm } from "@/contexts/confirm-context";
import {
    useAmendMutation,
    useCherryPickMutation,
    useCheckoutMutation,
    useCreateBranchMutation,
    useCreateTagMutation,
    useResetMutation,
    useRevertMutation,
} from "@/lib/backend/mutations/repository-mutations";
import { summarizeWorkflowOutcome } from "@/lib/backend/mutations/workflow-outcome";
import type {
    CommitSummary,
    ResetKind,
    WorkflowOutcome,
} from "@/lib/backend/protocol";
import { toastError } from "@/lib/toast-error";

const MAX_LISTED_CONFLICT_PATHS = 5;

export interface TagFormInput {
    name: string;
    message?: string;
}

function shortSha(commit: CommitSummary): string {
    return commit.id.slice(0, 7);
}

function resetConfirmCopy(
    kind: ResetKind,
    short: string
): { description: React.ReactNode; destructive: boolean } {
    switch (kind) {
        case "soft":
            return {
                description: `The branch moves to ${short} and every commit after it becomes staged changes.`,
                destructive: false,
            };
        case "mixed":
            return {
                description: `The branch moves to ${short} and every commit after it becomes unstaged changes.`,
                destructive: false,
            };
        case "hard":
            return {
                description: `The branch moves to ${short}, commits after it are removed, and all uncommitted changes are discarded. This cannot be undone.`,
                destructive: true,
            };
    }
}

function reportWorkflowOutcome(
    outcome: WorkflowOutcome,
    title: string,
    successDescription: React.ReactNode
): void {
    const summary = summarizeWorkflowOutcome(outcome);
    if (summary.conflictPaths.length > 0) {
        const shown = summary.conflictPaths.slice(0, MAX_LISTED_CONFLICT_PATHS);
        const rest = summary.conflictPaths.length - shown.length;
        toastManager.add({
            title: `${title} stopped on conflicts`,
            description: `${shown.join("\n")}${rest > 0 ? ` (+${rest} more)` : ""}`,
            type: "warning",
        });
        return;
    }
    toastManager.add({
        title,
        description: successDescription,
        type: "success",
    });
}

/**
 * Per-commit operations behind the history context menu: clipboard copies,
 * tag and branch creation, cherry-pick, revert, reset, checkout, undo, and
 * amend. Destructive flows confirm first; failures toast.
 */
export function useCommitActions(repoId: number | undefined) {
    const reset = useResetMutation(repoId ?? 0);
    const amend = useAmendMutation(repoId ?? 0);
    const checkout = useCheckoutMutation(repoId ?? 0);
    const revert = useRevertMutation(repoId ?? 0);
    const cherryPick = useCherryPickMutation(repoId ?? 0);
    const createTag = useCreateTagMutation(repoId ?? 0);
    const createBranch = useCreateBranchMutation(repoId ?? 0);
    const { confirm } = useConfirm();

    const copyText = async (text: string, title: string) => {
        try {
            await navigator.clipboard.writeText(text);
            toastManager.add({ title, type: "success" });
        } catch (error) {
            toastError("Could not copy", error);
        }
    };

    const copySha = (commit: CommitSummary) =>
        void copyText(commit.id, "Copied commit SHA");

    const copyMessage = (commit: CommitSummary) =>
        void copyText(commit.message, "Copied commit message");

    const resetTo = async (commit: CommitSummary, kind: ResetKind) => {
        const copy = resetConfirmCopy(kind, shortSha(commit));
        const result = await confirm({
            title: `Reset to ${shortSha(commit)}?`,
            description: copy.description,
            confirmText: "Reset",
            variant: copy.destructive ? "destructive" : "default",
        });
        if (!result.confirmed) return;
        try {
            await reset.mutateAsync({ kind, target: commit.id });
        } catch (error) {
            toastError("Could not reset", error);
        }
    };

    const undoCommit = async (commit: CommitSummary) => {
        const parent = commit.parentIds[0];
        if (!parent) return;
        const result = await confirm({
            title: "Undo commit?",
            description: `Changes from "${commit.summaryLine}" return to the staging area and the commit leaves the branch.`,
            confirmText: "Undo commit",
        });
        if (!result.confirmed) return;
        try {
            await reset.mutateAsync({ kind: "soft", target: parent });
        } catch (error) {
            toastError("Could not undo commit", error);
        }
    };

    const checkoutCommit = async (commit: CommitSummary) => {
        const result = await confirm({
            title: `Checkout ${shortSha(commit)}?`,
            description:
                "HEAD detaches to this commit. Commits made here are not on any branch until you create one.",
            confirmText: "Checkout",
        });
        if (!result.confirmed) return;
        try {
            await checkout.mutateAsync(commit.id);
        } catch (error) {
            toastError("Could not checkout commit", error);
        }
    };

    const revertCommit = async (commit: CommitSummary) => {
        try {
            const outcome = await revert.mutateAsync({
                target: commit.id,
                // Merge commits revert against the first parent.
                parentIndex: commit.parentIds.length > 1 ? 1 : undefined,
            });
            reportWorkflowOutcome(
                outcome,
                "Revert",
                `Reverted "${commit.summaryLine}"`
            );
        } catch (error) {
            toastError("Revert failed", error);
        }
    };

    const cherryPickCommit = async (commit: CommitSummary) => {
        try {
            const outcome = await cherryPick.mutateAsync({ target: commit.id });
            reportWorkflowOutcome(
                outcome,
                "Cherry-pick",
                `Cherry-picked "${commit.summaryLine}"`
            );
        } catch (error) {
            toastError("Cherry-pick failed", error);
        }
    };

    const createTagHere = async (
        commit: CommitSummary,
        input: TagFormInput
    ): Promise<boolean> => {
        try {
            await createTag.mutateAsync({
                name: input.name,
                target: commit.id,
                message: input.message || undefined,
            });
            return true;
        } catch (error) {
            toastError("Could not create tag", error);
            return false;
        }
    };

    const createBranchHere = async (
        commit: CommitSummary,
        name: string
    ): Promise<boolean> => {
        try {
            await createBranch.mutateAsync({
                name,
                startPoint: commit.id,
                checkout: false,
            });
            return true;
        } catch (error) {
            toastError("Could not create branch", error);
            return false;
        }
    };

    const amendCommit = async (message: string): Promise<boolean> => {
        try {
            await amend.mutateAsync({ message });
            return true;
        } catch (error) {
            toastError("Could not amend commit", error);
            return false;
        }
    };

    return {
        busy:
            reset.isPending ||
            amend.isPending ||
            checkout.isPending ||
            revert.isPending ||
            cherryPick.isPending ||
            createTag.isPending ||
            createBranch.isPending,
        copySha,
        copyMessage,
        resetTo,
        undoCommit,
        checkoutCommit,
        revertCommit,
        cherryPickCommit,
        createTagHere,
        createBranchHere,
        amendCommit,
    };
}
