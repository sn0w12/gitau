import type { GraphQuery } from "../protocol";
import { GraphSessionController } from "./graph-session";
import { createSessionRegistry } from "./session-registry";

let sessionSeq = 0;

interface AcquireInput {
    repoId: number;
    tabId?: string;
    query?: GraphQuery;
    generation?: number;
    /** Begin the stream on acquire, folding prefetch into one call. */
    start?: boolean;
}

/**
 * Process-wide registry of live commit-graph sessions, keyed like the
 * diff registry: repo plus tab plus query plus generation. A generation
 * advance re-acquires a fresh session so mounted graphs re-stream after
 * changes instead of reusing a warm key.
 */
const inner = createSessionRegistry<GraphSessionController, AcquireInput>({
    create: (backend, input) =>
        new GraphSessionController(backend, {
            sessionId: `graph-${Date.now().toString(36)}-${++sessionSeq}`,
            repoId: input.repoId,
            tabId: input.tabId,
            query: input.query,
        }),
    key: (input) =>
        JSON.stringify({
            repoId: input.repoId,
            tabId: input.tabId ?? null,
            query: input.query ?? null,
            generation: input.generation ?? null,
        }),
});

function acquire(
    backend: Parameters<typeof inner.acquire>[0],
    input: AcquireInput
): GraphSessionController {
    const controller = inner.acquire(backend, input);
    if (input.start) controller.begin();
    return controller;
}

export const graphSessionRegistry = {
    acquire,
    release: inner.release,
    markRepoStale: inner.markRepoStale,
    disposeAllForTab: inner.disposeAllForTab,
    disposeAllForRepo: inner.disposeAllForRepo,
    resetForTests: inner.resetForTests,
};
