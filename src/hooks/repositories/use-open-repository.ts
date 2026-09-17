import { useCallback } from "react";

import { useActiveTabRouter } from "@/hooks/tabs/use-active-tab-router";
import { openRepositoryByPath } from "@/lib/repositories/open-repository";
import { toastError } from "@/lib/toast-error";
import { associateTabWithRepoPath, appStore } from "@/stores/app-store";

// Binds the active tab to the repo so closing the tab releases the backend.
export function useOpenRepository() {
    const router = useActiveTabRouter();

    return useCallback(
        async (repoPath: string) => {
            const result = await openRepositoryByPath(repoPath);
            if (result.status === "failed") {
                toastError("Could not open repository", result.error);
                return null;
            }
            const activeTabId = appStore.state.activeTabId;
            if (activeTabId) {
                associateTabWithRepoPath(activeTabId, repoPath);
            }
            void router?.navigate({ to: `/repo/${result.repoId}` });
            return result.repoId;
        },
        [router]
    );
}
