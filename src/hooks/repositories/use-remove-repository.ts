import { useCallback } from "react";

import { useConfirm } from "@/contexts/confirm-context";
import { useAppServices } from "@/contexts/services-context";
import { repoDisplayName } from "@/hooks/repositories/use-repo-identity";
import { disposeStreamsForRepo } from "@/lib/backend/streams/unified-streams";
import type { GitBackendError } from "@/lib/backend/transport/invoke";
import {
    closeTabFully,
    forgetClosedTabsForRepo,
} from "@/lib/routing/tab-lifecycle";
import type { TabRouter } from "@/lib/routing/tab-router-factory";
import { trashBinName } from "@/lib/utils";
import type { TabRecord } from "@/stores/app-store";
import { appStore, openTab, updateTab } from "@/stores/app-store";
import { getEntryByPath, removeRepo } from "@/stores/repository-store";
import { disposeRuntime, getRuntime } from "@/stores/tab-runtime";

export type RemoveRepositoryOutcome =
    | { status: "removed" }
    | { status: "cancelled" }
    | { status: "failed"; error: GitBackendError };

interface RemovalTabPlan {
    survivorId: string | null;
    boundTabIds: string[];
}

/** Picks a survivor from the tabs bound to the repo being removed. When every
 * tab belongs to the repo, the last bound tab is kept and unbound; otherwise
 * existing unrelated tabs already provide the shell's survivor. */
function planRemovalTabs(
    tabs: readonly TabRecord[],
    repoPath: string
): RemovalTabPlan {
    const boundTabIds = tabs
        .filter((tab) => tab.repoPath?.toLowerCase() === repoPath.toLowerCase())
        .map((tab) => tab.tabId);
    const keepSurvivor = boundTabIds.length === tabs.length;
    return {
        survivorId: keepSurvivor ? boundTabIds[boundTabIds.length - 1] : null,
        boundTabIds,
    };
}

/** Reopens tabs that are no longer in the store (closed or dropped) so a
 * removal rollback restores the previous shell layout. */
function reopenMissingTabs(tabs: readonly TabRecord[]): void {
    for (const tab of tabs) {
        if (
            !appStore.state.tabs.some((current) => current.tabId === tab.tabId)
        ) {
            openTab(tab);
        }
    }
}

/** With no separate survivor, reopen the unrelated tabs that were touched and
 * send the leftover bound tab home. */
function prepareFallbackShell(
    boundTabIds: readonly string[],
    tabsBeforeRemoval: readonly TabRecord[],
    repoPath: string
): void {
    reopenMissingTabs(
        tabsBeforeRemoval.filter(
            (tab) => tab.repoPath?.toLowerCase() !== repoPath.toLowerCase()
        )
    );
    updateTab(boundTabIds[boundTabIds.length - 1], {
        repoPath: undefined,
        lastResolvedHref: "/",
    });
}

/** Reopens tabs that were closed for this removal when the backend call fails. */
function restoreClosedTabs(
    tabsBeforeRemoval: readonly TabRecord[],
    closedTabIds: ReadonlySet<string>
): void {
    reopenMissingTabs(
        tabsBeforeRemoval.filter((tab) => closedTabIds.has(tab.tabId))
    );
}

async function navigateTabHome(tabId: string, router: TabRouter | null) {
    updateTab(tabId, { repoPath: undefined, lastResolvedHref: "/" });
    if (router) await router.navigate({ to: "/", replace: true });
}

async function navigateLiveTabHome(tabId: string) {
    updateTab(tabId, { repoPath: undefined, lastResolvedHref: "/" });
    const router = getRuntime(tabId)?.router;
    if (router) await router.navigate({ to: "/", replace: true });
}

/**
 * Confirms and removes a repository from the app: the backend closes the
 * session and optionally trashes the working copy, then the store entry
 * and every tab bound to the repo are dropped. The shell always keeps one
 * tab, so when every tab belongs to the removed repo the survivor is
 * unbound and navigated home instead.
 */
export function useRemoveRepository() {
    const { confirm } = useConfirm();
    const { backend } = useAppServices();

    return useCallback(
        async (repoPath: string): Promise<RemoveRepositoryOutcome> => {
            const name = repoDisplayName(repoPath);
            const bin = trashBinName();

            const result = await confirm({
                title: `Remove ${name}?`,
                description: (checked) =>
                    checked
                        ? `${name} will be moved to the ${bin} and removed from gitau.`
                        : `${name} will be removed from gitau. The working copy stays on disk.`,
                checkbox: { label: `Move to ${bin}` },
                confirmText: "Remove",
                variant: "destructive",
            });
            if (!result.confirmed) return { status: "cancelled" };

            const moveToTrash = result.checkboxChecked === true;

            // Close every tab first. This disposes diff sessions, cancels
            // tab-owned work, releases runtimes, and closes the backend repo
            // binding when it is no longer referenced. Windows cannot recycle
            // a directory while the renderer still owns repo resources.
            const tabsBeforeRemoval = appStore.state.tabs;
            const { survivorId, boundTabIds } = planRemovalTabs(
                tabsBeforeRemoval,
                repoPath
            );
            const closedTabIds = new Set<string>();

            for (const tabId of boundTabIds) {
                if (tabId === survivorId) continue;
                await closeTabFully(tabId, backend);
                closedTabIds.add(tabId);
            }
            if (survivorId === null && boundTabIds.length > 0) {
                prepareFallbackShell(boundTabIds, tabsBeforeRemoval, repoPath);
            }

            if (survivorId !== null) {
                await disposeRuntime(survivorId);
            }
            const entry = getEntryByPath(repoPath);
            if (entry?.repoId !== undefined) {
                disposeStreamsForRepo(entry.repoId);
            }

            const removal = await backend.repositories.remove(
                repoPath,
                moveToTrash
            );
            if (!removal.ok) {
                restoreClosedTabs(tabsBeforeRemoval, closedTabIds);
                return { status: "failed", error: removal.error };
            }

            removeRepo(repoPath);

            // The survivor runtime is disposed before the backend call, so
            // capture its router before teardown and navigate the live memory
            // history after the repository has been removed.
            const survivorRouter = survivorId
                ? (getRuntime(survivorId)?.router ?? null)
                : null;
            forgetClosedTabsForRepo(repoPath);

            if (survivorId !== null) {
                await navigateTabHome(survivorId, survivorRouter);
            } else if (boundTabIds.length > 0) {
                // If the shell has no separate survivor, explicitly activate
                // the home route on the remaining tab rather than leaving the
                // router's memory history at the deleted repo route.
                const remaining = appStore.state.tabs[0];
                if (remaining) await navigateLiveTabHome(remaining.tabId);
            }

            return { status: "removed" };
        },
        [confirm, backend]
    );
}
