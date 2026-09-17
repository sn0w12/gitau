import { useSelector } from "@tanstack/react-store";
import { useEffect, useRef } from "react";

import { useAppServices } from "@/contexts/services-context";
import type { DiffRequest } from "@/lib/backend/protocol";
import type {
    DiffSessionController,
    DiffSessionState,
} from "@/lib/backend/streams/diff-session";
import {
    acquireDiff,
    releaseStream,
} from "@/lib/backend/streams/unified-streams";
import { getRuntime, setActiveDiffSession } from "@/stores/tab-runtime";

/**
 * Opens (or reuses) a streaming diff session keyed by request identity:
 * remounts and hover prefetches share one stream; switching requests
 * releases the previous reference, and unmount releases the last one.
 * Acquired synchronously during render so a warm session's rows are
 * present on first paint; registry entries are IPC stateless so the
 * backend captured at first acquisition stays valid for their lifetime.
 */
export type DiffSessionResult = DiffSessionState & {
    controller: DiffSessionController;
};

export function useDiffSession(
    input: { repoId: number; request?: DiffRequest; generation?: number },
    tab?: { tabId: string }
): DiffSessionResult {
    const { backend } = useAppServices();
    const repoKey = input.repoId;
    const tabKey = tab?.tabId;
    const request = input.request;
    const generation = input.generation;

    // The generation is part of the identity only when the caller supplies it
    // (worktree-scoped views). A bump then forces a fresh session and a new
    // stream, so a diff never lingers on an out-of-date snapshot. Commit
    // diffs omit it: they are immutable and should reuse the warm cache.
    const key = JSON.stringify({
        repoId: repoKey,
        tabId: tabKey ?? null,
        request: request ?? null,
        generation: generation ?? null,
    });

    const held = useRef<{
        key: string;
        controller: DiffSessionController;
    } | null>(null);

    /* Render-phase binding to an external (registry-owned) resource is the
     * only way to guarantee warm rows before paint; React's ref is the only
     * slot that survives across renders here. */
    /* oxlint-disable react/refs */
    if (held.current === null || held.current.key !== key) {
        if (held.current !== null) {
            releaseStream(held.current.controller);
        }
        const acquired = acquireDiff(backend, {
            repoId: repoKey,
            tabId: tabKey,
            request,
            generation,
            start: true,
        });
        held.current = { key, controller: acquired };
    }
    const controller = held.current.controller;
    useEffect(() => {
        if (tabKey) {
            const runtime = getRuntime(tabKey);
            if (runtime)
                setActiveDiffSession(runtime.tabStore, controller.sessionId);
        }
    }, [controller, tabKey]);

    useEffect(() => {
        return () => releaseStream(controller);
    }, [controller]);

    const sessionState = useSelector(controller.store, (state) => state);
    /* oxlint-disable-next-line react/refs */
    return { ...sessionState, controller };
}
