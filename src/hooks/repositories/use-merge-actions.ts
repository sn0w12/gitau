import { toastManager } from "@/components/ui/toast";
import { useConfirm } from "@/contexts/confirm-context";
import {
    useMergeAbortMutation,
    useMergeContinueMutation,
    useMergeMutation,
    useResolveConflictMutation,
} from "@/lib/backend/mutations/repository-mutations";
import { summarizeWorkflowOutcome } from "@/lib/backend/mutations/workflow-outcome";
import type { ResolveSide, WorkflowOutcome } from "@/lib/backend/protocol";
import { toastError } from "@/lib/toast-error";

const MAX_LISTED_CONFLICT_PATHS = 5;

function reportMergeOutcome(outcome: WorkflowOutcome): void {
    const summary = summarizeWorkflowOutcome(outcome);
    if (summary.conflictPaths.length > 0) {
        const shown = summary.conflictPaths.slice(0, MAX_LISTED_CONFLICT_PATHS);
        const rest = summary.conflictPaths.length - shown.length;
        toastManager.add({
            title: "Merge stopped on conflicts",
            description: `${shown.join("\n")}${rest > 0 ? ` (+${rest} more)` : ""}`,
            type: "warning",
        });
        return;
    }
    toastManager.add({
        title: "Merge finished",
        description: summary.message,
        type: "success",
    });
}

export function useMergeActions(repoId: number) {
    const merge = useMergeMutation(repoId);
    const mergeContinue = useMergeContinueMutation(repoId);
    const mergeAbort = useMergeAbortMutation(repoId);
    const resolve = useResolveConflictMutation(repoId);
    const { confirm } = useConfirm();

    const mergeBranch = async (target: string) => {
        try {
            const outcome = await merge.mutateAsync({ target });
            reportMergeOutcome(outcome);
            return outcome;
        } catch (error) {
            toastError("Merge failed", error);
            return null;
        }
    };

    const continueMerge = async (message?: string) => {
        try {
            const outcome = await mergeContinue.mutateAsync({ message });
            if (outcome.outcome === "conflicted") {
                reportMergeOutcome(outcome);
            } else {
                toastManager.add({
                    title: "Merge committed",
                    description: summarizeWorkflowOutcome(outcome).message,
                    type: "success",
                });
            }
            return outcome;
        } catch (error) {
            toastError("Could not finish merge", error);
            return null;
        }
    };

    const abortMerge = async () => {
        const result = await confirm({
            title: "Abort merge?",
            description:
                "The merge stops and all uncommitted changes are discarded. This cannot be undone.",
            confirmText: "Abort merge",
            variant: "destructive",
        });
        if (!result.confirmed) return null;
        try {
            const outcome = await mergeAbort.mutateAsync();
            toastManager.add({
                title: "Merge aborted",
                type: "success",
            });
            return outcome;
        } catch (error) {
            toastError("Could not abort merge", error);
            return null;
        }
    };

    const resolveConflict = async (
        path: string,
        side: ResolveSide
    ): Promise<boolean> => {
        try {
            await resolve.mutateAsync({ path, side });
            return true;
        } catch (error) {
            toastError(
                side === "ours"
                    ? "Could not take ours"
                    : "Could not take theirs",
                error
            );
            return false;
        }
    };

    return {
        mergeBranch,
        continueMerge,
        abortMerge,
        resolveConflict,
        busy:
            merge.isPending ||
            mergeContinue.isPending ||
            mergeAbort.isPending ||
            resolve.isPending,
        pending:
            merge.isPending || mergeContinue.isPending || mergeAbort.isPending,
    };
}
