import { useSelector } from "@tanstack/react-store";
import { useCallback, useSyncExternalStore } from "react";

import type { TabRouter } from "@/lib/routing/tab-router-factory";
import { selectActiveTab, appStore } from "@/stores/app-store";
import { getRuntime, runtimeVersionStore } from "@/stores/tab-runtime";

function routerSnapshot(tabId: string): TabRouter | null {
    return getRuntime(tabId)?.router ?? null;
}

function historyIndex(router: TabRouter | null): number {
    return router?.history.location.state.__TSR_index ?? 0;
}

function historyLength(router: TabRouter | null): number {
    return router?.history.length ?? 1;
}

/** Read-only view of the active tab's router; runtime creation belongs to
 * ActiveTabHost (a store update during render here would break React). */
export function useActiveTabRouter(): TabRouter | null {
    const activeTab = useSelector(appStore, selectActiveTab);
    const tabId = activeTab?.tabId;

    const subscribe = useCallback(
        (onStoreChange: () => void) =>
            runtimeVersionStore().subscribe(onStoreChange).unsubscribe,
        []
    );
    const getSnapshot = useCallback(
        () => (tabId === undefined ? null : routerSnapshot(tabId)),
        [tabId]
    );

    return useSyncExternalStore(subscribe, getSnapshot);
}

/** Reactive view over the active tab's memory history: memory histories
 * truncate forward entries on push, so forward availability is exactly
 * "entries ahead of us". */
export function useActiveTabHistory(): {
    canGoBack: boolean;
    canGoForward: boolean;
    back: () => void;
    forward: () => void;
} {
    const router = useActiveTabRouter();

    const subscribe = useCallback(
        (onStoreChange: () => void) =>
            router ? router.history.subscribe(onStoreChange) : () => {},
        [router]
    );
    const index = useSyncExternalStore(subscribe, () => historyIndex(router));
    const length = useSyncExternalStore(subscribe, () => historyLength(router));

    return {
        canGoBack: index > 0,
        canGoForward: index < length - 1,
        back: () => router?.history.back(),
        forward: () => router?.history.forward(),
    };
}
