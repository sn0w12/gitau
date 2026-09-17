import type { BackendClient } from "../transport/client";

/**
 * Completed (or still-streaming) sessions keep their rows warm so remounting
 * the same request paints instantly; failed or cancelled sessions hold no
 * reusable content and are dropped as soon as their last consumer leaves.
 */
const MAX_RETAINED_SESSIONS = 8;

interface RegistryEntry<C> {
    controller: C;
    refs: number;
}

export interface RegistryControllerShape {
    sessionId: string;
    isDisposed(): boolean;
    dispose(): void;
}

export function isRetainableStatus(status: string): boolean {
    return status === "running" || status === "completed" || status === "stale";
}

export function createSessionRegistry<
    C extends {
        sessionId: string;
        store: { state: { repoId: number; status: string } };
        config: { tabId?: string };
        isDisposed(): boolean;
        dispose(): void;
        markStale(): void;
    },
    AcquireInput extends { repoId: number; tabId?: string },
>(options: {
    create(backend: BackendClient, input: AcquireInput): C;
    key(input: AcquireInput): string;
}) {
    /** Insertion order doubles as LRU order; acquire refreshes recency. */
    const entries = new Map<string, RegistryEntry<C>>();

    function acquire(backend: BackendClient, input: AcquireInput): C {
        const key = options.key(input);
        const existing = entries.get(key);
        if (existing) {
            if (!existing.controller.isDisposed()) {
                entries.delete(key);
                entries.set(key, existing);
                existing.refs += 1;
                return existing.controller;
            }
            entries.delete(key);
        }

        const controller = options.create(backend, input);
        entries.set(key, { controller, refs: 1 });
        return controller;
    }

    function release(controller: C): void {
        for (const [key, entry] of entries) {
            if (entry.controller !== controller) continue;
            entry.refs = Math.max(0, entry.refs - 1);
            if (
                entry.refs === 0 &&
                !isRetainableStatus(entry.controller.store.state.status)
            ) {
                entries.delete(key);
                entry.controller.dispose();
            } else {
                evictRetained();
            }
            return;
        }
    }

    function markRepoStale(repoId: number): void {
        for (const entry of entries.values()) {
            if (entry.controller.store.state.repoId === repoId)
                entry.controller.markStale();
        }
    }

    function disposeAllForTab(tabId: string): void {
        for (const [key, entry] of entries) {
            if (entry.controller.config.tabId === tabId) {
                entries.delete(key);
                entry.controller.dispose();
            }
        }
    }

    function disposeAllForRepo(repoId: number): void {
        for (const [key, entry] of entries) {
            if (entry.controller.store.state.repoId === repoId) {
                entries.delete(key);
                entry.controller.dispose();
            }
        }
    }

    function getBySessionId(sessionId: string): C | undefined {
        for (const entry of entries.values()) {
            if (entry.controller.sessionId === sessionId)
                return entry.controller;
        }
        return undefined;
    }

    function removeBySessionId(sessionId: string): void {
        for (const [key, entry] of entries) {
            if (entry.controller.sessionId === sessionId) {
                entries.delete(key);
                entry.controller.dispose();
                return;
            }
        }
    }

    /** Disposes oldest zero-ref sessions beyond the retention cap. */
    function evictRetained(): void {
        const reclaimable: string[] = [];
        for (const [key, entry] of entries) {
            const status = entry.controller.store.state.status;
            if (entry.refs === 0 && isRetainableStatus(status))
                reclaimable.push(key);
        }
        for (let i = 0; i < reclaimable.length - MAX_RETAINED_SESSIONS; i++) {
            const key = reclaimable[i];
            const entry = entries.get(key);
            if (!entry) continue;
            entries.delete(key);
            entry.controller.dispose();
        }
    }

    /** Test helper: drops every session without cancellation side effects. */
    function resetForTests(): void {
        for (const entry of entries.values()) entry.controller.dispose();
        entries.clear();
    }

    return {
        acquire,
        release,
        getBySessionId,
        removeBySessionId,
        markRepoStale,
        disposeAllForTab,
        disposeAllForRepo,
        resetForTests,
    };
}
