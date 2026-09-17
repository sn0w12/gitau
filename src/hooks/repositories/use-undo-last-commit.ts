import { useQueryClient } from "@tanstack/react-query";
import { useCallback } from "react";

import { toastManager } from "@/components/ui/toast";
import { useConfirm } from "@/contexts/confirm-context";
import { useAppServices } from "@/contexts/services-context";
import { useResetMutation } from "@/lib/backend/mutations/repository-mutations";
import type { HistoryPage, RepoSnapshot } from "@/lib/backend/protocol";
import { repositoryKeys } from "@/lib/backend/queries/query-keys";
import { historyPageQuery } from "@/lib/backend/queries/repository-queries";
import { toastError } from "@/lib/toast-error";

/**
 * GitHub Desktop-style "undo last commit": one confirmation, then a soft
 * reset to the parent so the commit's changes return staged. Reads HEAD
 * from the snapshot cache and the parent from the first history page.
 */
export function useUndoLastCommit(
    repoId: number | undefined
): () => Promise<void> {
    const { backend } = useAppServices();
    const queryClient = useQueryClient();
    const { confirm } = useConfirm();
    const reset = useResetMutation(repoId ?? 0);

    return useCallback(async () => {
        if (repoId === undefined) return;

        const snapshot = queryClient.getQueryData<RepoSnapshot>(
            repositoryKeys.snapshot(repoId)
        );
        const head = snapshot?.head;
        if (!head || head.state !== "attached") {
            toastManager.add({
                title: "Nothing to undo",
                description:
                    "HEAD is not on a branch, so there is no last commit to undo.",
                type: "info",
            });
            return;
        }

        const page: HistoryPage = await queryClient.fetchQuery(
            historyPageQuery({ backend }, repoId, { limit: 1 })
        );
        const headCommit = page.commits[0];
        const parent = headCommit?.parentIds[0];
        if (!headCommit || !parent) {
            // Unborn HEAD or a root commit: nothing before it to reset to.
            toastManager.add({
                title: "Nothing to undo",
                description: "There is no commit to undo on this branch.",
                type: "info",
            });
            return;
        }

        const result = await confirm({
            title: "Undo last commit?",
            description: `Changes from "${headCommit.summaryLine}" return to the staging area and the commit leaves ${head.branch}.`,
            confirmText: "Undo commit",
        });
        if (!result.confirmed) return;

        try {
            await reset.mutateAsync({ kind: "soft", target: parent });
        } catch (error) {
            toastError("Could not undo commit", error);
        }
    }, [repoId, backend, queryClient, confirm, reset]);
}
