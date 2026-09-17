import { disposeStreamsForTab } from "@/lib/backend/streams/unified-streams";
import type { BackendClient } from "@/lib/backend/transport/client";
import { activateTab, appStore, closeTab, openTab } from "@/stores/app-store";
import type { TabRecord } from "@/stores/app-store";
import { forgetOperationsForTab } from "@/stores/operation-store";
import { getEntryByPath, unbindRepo } from "@/stores/repository-store";
import { getSetting } from "@/stores/settings-store";
import { disposeRuntime, getRuntime } from "@/stores/tab-runtime";

/** Most recently closed tabs first; in-memory only, reset on app restart. */
const closedTabStack: TabRecord[] = [];

/**
 * Full tab close: cancels owned cancellable work (diffs), disposes the
 * runtime, drops the tab record, and closes the backend repository when
 * the last tab referencing it disappears.
 */
export async function closeTabFully(
    tabId: string,
    backend?: BackendClient
): Promise<void> {
    const closing = getRuntime(tabId)?.tab;

    for (const operationId of forgetOperationsForTab(tabId)) {
        // Best-effort: the client never rejects; terminal states also arrive
        // via each diff session's own stream.
        const cancelled = await backend?.diff.cancel(operationId);
        void cancelled;
    }
    disposeStreamsForTab(tabId);

    if (closing) {
        closedTabStack.push(closing);
    }

    closeTab(tabId);
    await disposeRuntime(tabId);

    if (!closing?.repoPath) return;
    if (!getSetting("closeRepoWithLastTab")) return;

    const stillReferenced = appStore.state.tabs.some(
        (tab) => tab.repoPath === closing.repoPath
    );
    if (stillReferenced) return;

    // Resolve the live backend binding from the repository store; when the
    // repo was never opened this session there is nothing to close.
    const boundId = getEntryByPath(closing.repoPath)?.repoId;

    // A failed close is harmless: the backend reaps idle repositories.
    if (boundId !== undefined) {
        const closed = await backend?.repositories.close(boundId);
        void closed;
    }
    // Keep the repo listed in the sidebar; only drop the process-local
    // binding so the next use reopens it fresh.
    unbindRepo(boundId ?? -1);
}

async function closeTabsFully(
    tabIds: string[],
    backend?: BackendClient
): Promise<void> {
    for (const tabId of tabIds) {
        await closeTabFully(tabId, backend);
    }
}

export async function closeOtherTabsFully(
    keepTabId: string,
    backend?: BackendClient
): Promise<void> {
    const otherIds = appStore.state.tabs
        .filter((tab) => tab.tabId !== keepTabId)
        .map((tab) => tab.tabId);

    await closeTabsFully(otherIds, backend);

    if (appStore.state.tabs.some((tab) => tab.tabId === keepTabId)) {
        activateTab(keepTabId);
    }
}

export async function closeTabsToRightFully(
    tabId: string,
    backend?: BackendClient
): Promise<void> {
    const index = appStore.state.tabs.findIndex((tab) => tab.tabId === tabId);
    if (index === -1) return;

    const rightIds = appStore.state.tabs
        .slice(index + 1)
        .map((tab) => tab.tabId);

    await closeTabsFully(rightIds, backend);
}

/** Reopens the most recently closed tab, or returns null when empty. */
export function reopenLastClosedTab(): TabRecord | null {
    // Never resurrect a duplicate id if something re-opened it meanwhile.
    while (closedTabStack.length > 0) {
        const record = closedTabStack.pop();
        if (!record) break;
        if (appStore.state.tabs.some((tab) => tab.tabId === record.tabId)) {
            continue;
        }
        openTab(record);
        return record;
    }
    return null;
}

/** Drops closed-tab records for a removed repository so reopen cannot
 * resurrect a tab pointing at a repo that no longer exists. */
export function forgetClosedTabsForRepo(repoPath: string): void {
    for (let index = closedTabStack.length - 1; index >= 0; index -= 1) {
        if (closedTabStack[index].repoPath === repoPath) {
            closedTabStack.splice(index, 1);
        }
    }
}

/** Test helper. */
export function clearClosedTabStackForTests(): void {
    closedTabStack.length = 0;
}
