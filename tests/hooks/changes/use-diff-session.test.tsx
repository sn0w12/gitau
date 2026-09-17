import { renderHook, act, cleanup } from "@testing-library/react";
// @vitest-environment jsdom
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";

import { useDiffSession } from "@/hooks/changes/use-diff-session";
import type { DiffEvent } from "@/lib/backend/protocol";
import { diffSessionRegistry } from "@/lib/backend/streams/diff-session-registry";
import type { BackendClient } from "@/lib/backend/transport/client";

vi.mock("@/contexts/services-context", () => ({
    useAppServices: () => ({ backend: fakeBackendRef.backend }),
}));

const fakeBackendRef: { backend: BackendClient } = {
    backend: undefined as unknown as BackendClient,
};

/** Emits a full single-section stream synchronously on open. */
function makeInstantBackend(): BackendClient {
    return {
        diff: {
            open: (
                _repoId: number,
                _request: unknown,
                onEvent: (event: DiffEvent) => void
            ) => {
                const events: DiffEvent[] = [
                    {
                        event: "started",
                        operationId: 1,
                        snapshotId: 0,
                        generation: 1,
                        sections: [
                            {
                                sectionId: 0,
                                path: "warm.rs",
                                kind: "modified",
                                binary: false,
                                complete: false,
                            },
                        ],
                        estimatedTotalRows: 1,
                    },
                    {
                        event: "sectionLayout",
                        operationId: 1,
                        sectionId: 0,
                        startRow: 0,
                        rowCount: 1,
                    },
                    {
                        event: "chunk",
                        operationId: 1,
                        sectionId: 0,
                        rowStart: 0,
                        rows: [
                            {
                                kind: "addition",
                                newLineno: 1,
                                content: "fn warm() {}",
                            },
                        ],
                    },
                    {
                        event: "completed",
                        operationId: 1,
                        totalRows: 1,
                        additions: 1,
                        deletions: 0,
                        durationMs: 0,
                    },
                ];
                for (const event of events) onEvent(event);
                return Promise.resolve(1);
            },
            readRange: () =>
                Promise.resolve({
                    rows: [],
                    nextCursor: 0,
                    hasMore: false,
                    knownTotalRows: 0,
                    complete: true,
                }),
            cancel: () => Promise.resolve(true),
        },
    } as unknown as BackendClient;
}

describe("useDiffSession synchronous acquisition", () => {
    beforeEach(() => {
        diffSessionRegistry.resetForTests();
        fakeBackendRef.backend = makeInstantBackend();
    });

    afterEach(() => {
        cleanup();
        diffSessionRegistry.resetForTests();
        vi.restoreAllMocks();
    });

    it("exposes warm session content on the very first render", async () => {
        // Warm the registry exactly the way a prior mount would have.
        let release!: () => void;
        await act(async () => {
            release = diffSessionRegistry.prefetch(fakeBackendRef.backend, {
                repoId: 1,
            });
            await Promise.resolve();
        });
        release();

        const seenSections: number[] = [];
        const seenStatuses: string[] = [];
        const { unmount } = renderHook(() => {
            const state = useDiffSession({ repoId: 1 });
            seenSections.push(state.sections.length);
            seenStatuses.push(state.status);
            return state;
        });

        expect(seenSections[0]).toBeGreaterThan(0);
        expect(seenStatuses[0]).toBe("completed");
        unmount();
    });

    it("re-acquires a fresh session when the generation advances", async () => {
        const generationRef = { current: 1 };
        const first = renderHook(() =>
            useDiffSession({ repoId: 3, generation: generationRef.current })
        );
        const firstId = first.result.current.sessionId;
        await act(async () => {
            generationRef.current = 2;
            first.rerender();
            await Promise.resolve();
        });
        expect(first.result.current.sessionId).not.toBe(firstId);
        first.unmount();
    });

    it("reuses the same warm controller across rerenders", async () => {
        let releaseWarm!: () => void;
        await act(async () => {
            releaseWarm = diffSessionRegistry.prefetch(fakeBackendRef.backend, {
                repoId: 2,
            });
            await Promise.resolve();
        });
        releaseWarm();

        const { result, rerender } = renderHook(() =>
            useDiffSession({ repoId: 2 })
        );
        const first = result.current;
        await act(async () => {
            rerender();
            await Promise.resolve();
        });
        expect(result.current.sessionId).toBe(first.sessionId);
        expect(result.current.sections.length).toBeGreaterThan(0);
    });
});
