import { useSelector } from "@tanstack/react-store";
import { useEffect, useRef } from "react";

import { useAppServices } from "@/contexts/services-context";
import type {
    GraphSessionController,
    GraphSessionState,
} from "@/lib/backend/streams/graph-session";
import {
    acquireGraph,
    releaseStream,
} from "@/lib/backend/streams/unified-streams";

/**
 * Opens (or reuses) the streaming commit-graph session for a repo in a tab.
 * Acquired synchronously during render so a warm session's rows are present
 * on first paint (replacing an identity releases its previous reference);
 * the final reference is released on unmount. Returns both the reactive
 * state and the controller (for range reads). Registry entries are IPC
 * stateless so the backend captured at first acquisition stays valid.
 */
export function useGraphSession(input: { repoId: number; tabId?: string }): {
    state: GraphSessionState;
    controller: GraphSessionController;
} {
    const { backend } = useAppServices();
    const tabKey = input.tabId;
    const repoKey = input.repoId;

    const key = JSON.stringify({ repoId: repoKey, tabId: tabKey ?? null });

    const held = useRef<{
        key: string;
        controller: GraphSessionController;
    } | null>(null);

    /* See useDiffSession for why binding happens during render. */
    /* oxlint-disable react/refs */
    if (held.current === null || held.current.key !== key) {
        if (held.current !== null) {
            releaseStream(held.current.controller);
        }
        const acquired = acquireGraph(backend, {
            repoId: repoKey,
            tabId: tabKey,
            start: true,
        });
        held.current = { key, controller: acquired };
    }
    const controller = held.current.controller;

    useEffect(() => {
        return () => releaseStream(controller);
    }, [controller]);

    const state = useSelector(controller.store, (state) => state);
    // Suppression runs to end of file: the public shape exposes the
    // render-bound controller directly.
    return { state, controller };
}
