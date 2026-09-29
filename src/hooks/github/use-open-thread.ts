import { useCallback } from "react";

import { useActiveTabRouter } from "@/hooks/tabs/use-active-tab-router";
import { openRepositoryByPath } from "@/lib/repositories/open-repository";
import { toastError } from "@/lib/toast-error";
import { associateTabWithRepoPath, appStore } from "@/stores/app-store";

export type ThreadKind = "issue" | "pull";

/**
 * Opens a GitHub thread in-app. The repository is addressed by durable path,
 * so a thread in a repo that has never been clicked still opens: the backend
 * binding is established on demand and reused when the repo is already open.
 */
export function useOpenThread() {
    const router = useActiveTabRouter();

    return useCallback(
        async (repoPath: string, kind: ThreadKind, number: number) => {
            const result = await openRepositoryByPath(repoPath);
            if (result.status === "failed") {
                toastError("Could not open repository", result.error);
                return null;
            }
            const activeTabId = appStore.state.activeTabId;
            if (activeTabId) {
                associateTabWithRepoPath(activeTabId, repoPath);
            }
            void router?.navigate({
                to: `/repo/${result.repoId}/${kind}/${number}`,
            });
            return result.repoId;
        },
        [router]
    );
}
