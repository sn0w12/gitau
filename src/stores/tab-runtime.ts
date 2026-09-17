import { createStore } from "@tanstack/store";
import type { Store } from "@tanstack/store";

import type { TabRouter } from "@/lib/routing/tab-router-factory";

import { appStore } from "./app-store";
import type { TabRecord } from "./app-store";

/** Ephemeral per-tab UI state: neither URL-addressable nor persisted.
 * Durable tab data lives in appStore; repo-scoped state in the repository
 * store. */
export interface TabState {
    tabId: string;
    /** Active diff session id for this tab, if any. */
    activeDiffSessionId: string | null;
}

export type TabStore = Store<TabState>;

export function createTabStore(tab: TabRecord): TabStore {
    return createStore<TabState>({
        tabId: tab.tabId,
        activeDiffSessionId: null,
    });
}

export function setActiveDiffSession(
    store: TabStore,
    sessionId: string | null
) {
    store.setState((state) =>
        state.activeDiffSessionId === sessionId
            ? state
            : { ...state, activeDiffSessionId: sessionId }
    );
}

/**
 * Ephemeral per-tab runtime (router + transient store). Owns nothing
 * durable; identity is tabId, lifetime is the tab's. Never persisted.
 */
export interface TabRuntime {
    tab: TabRecord;
    tabStore: TabStore;
    /** Created lazily by the router factory; keeping it here means switching
     * tabs never recreates the router (memory history survives). Type-only
     * import, so no runtime cycle. */
    router?: TabRouter;
    disposers: Array<() => void>;
    disposed: boolean;
}

const runtimes = new Map<string, TabRuntime>();
let version = 0;

const versionStore = createStore(version);

function bump() {
    version += 1;
    versionStore.setState(() => version);
}

export function getOrCreateRuntime(tab: TabRecord): TabRuntime {
    const existing = runtimes.get(tab.tabId);
    if (existing) return existing;

    const runtime: TabRuntime = {
        tab,
        tabStore: createTabStore(tab),
        disposers: [],
        disposed: false,
    };
    runtimes.set(tab.tabId, runtime);
    bump();
    return runtime;
}

export function getRuntime(tabId: string): TabRuntime | undefined {
    return runtimes.get(tabId);
}

export function requireRuntime(tabId: string): TabRuntime {
    const runtime = runtimes.get(tabId);
    if (!runtime) throw new Error(`no runtime for ${tabId}`);
    return runtime;
}

export function attachRouter(tabId: string, router: TabRouter) {
    const runtime = requireRuntime(tabId);
    runtime.router = router;
}

/** Tears a tab down: frees per-tab resources and listeners. */
export async function disposeRuntime(tabId: string): Promise<void> {
    const runtime = runtimes.get(tabId);
    if (!runtime || runtime.disposed) return;
    runtime.disposed = true;

    for (const dispose of runtime.disposers.splice(0)) {
        try {
            dispose();
        } catch {
            // a failing listener must not block teardown of the rest
        }
    }

    runtimes.delete(tabId);
    bump();
}

/** Reactive version counter; components subscribe to re-read the map. */
export function runtimeVersionStore() {
    return versionStore;
}

export function allRuntimes(): TabRuntime[] {
    return [...runtimes.values()];
}

/** Test helper. */
export function resetRuntimesForTests() {
    runtimes.clear();
    version += 1;
    versionStore.setState(() => version);
    appStore.setState((state) => ({ ...state, tabs: [], activeTabId: null }));
}
