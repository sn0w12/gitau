import type { DiffRequest } from "../protocol";
import type { BackendClient } from "../transport/client";
import { DiffSessionController } from "./diff-session";
import { createSessionRegistry } from "./session-registry";

let sessionSeq = 0;

interface AcquireInput {
    repoId: number;
    tabId?: string;
    request?: DiffRequest;
    generation?: number;
    /** Begin the stream on acquire, folding prefetch into one call. */
    start?: boolean;
}

/**
 * Process-wide registry of live diff sessions, keyed by request identity.
 * Acquire/release is reference counted; zero-ref sessions linger as a warm
 * cache (LRU-capped) until evicted, disposed with their tab, or marked
 * stale via markRepoStale.
 *
 * Identity caveat: the key includes the process-local repoId, the owning
 * tabId, and (when given) the repository generation. A generation advance
 * re-acquires a fresh session so mounted diffs re-stream after changes;
 * anything that must reach the same stream later (e.g. hover prefetch) has
 * to pass an identical {repoId, tabId, request} shape (generation is only
 * set for worktree scoped views).
 */
const inner = createSessionRegistry<DiffSessionController, AcquireInput>({
    create: (backend, input) =>
        new DiffSessionController(backend, {
            sessionId: `diff-${Date.now().toString(36)}-${++sessionSeq}`,
            repoId: input.repoId,
            tabId: input.tabId,
            request: input.request,
        }),
    key: (input) =>
        JSON.stringify({
            repoId: input.repoId,
            tabId: input.tabId ?? null,
            request: input.request ?? null,
            generation: input.generation ?? null,
        }),
});

function acquire(
    backend: Parameters<typeof inner.acquire>[0],
    input: AcquireInput
): DiffSessionController {
    const controller = inner.acquire(backend, input);
    if (input.start) controller.begin();
    return controller;
}

export const diffSessionRegistry = {
    acquire,
    release: inner.release,
    prefetch(
        backend: BackendClient,
        input: { repoId: number; tabId?: string; request?: DiffRequest }
    ): () => void {
        const controller = acquire(backend, { ...input, start: true });
        return () => inner.release(controller);
    },
    get(sessionId: string): DiffSessionController | undefined {
        return inner.getBySessionId(sessionId);
    },
    remove(sessionId: string): void {
        inner.removeBySessionId(sessionId);
    },
    markRepoStale: inner.markRepoStale,
    disposeAllForTab: inner.disposeAllForTab,
    disposeAllForRepo: inner.disposeAllForRepo,
    resetForTests: inner.resetForTests,
};
