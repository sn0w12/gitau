import { describe, expect, it, beforeEach } from "vitest";

import type {
    DiffEvent,
    DiffRequest,
    RangeResult,
} from "@/lib/backend/protocol";
import { diffSessionRegistry } from "@/lib/backend/streams/diff-session-registry";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { GitBackendError } from "@/lib/backend/transport/invoke";
import type { Result } from "@/lib/backend/transport/result";

function fakeBackend() {
    const listeners: Array<(event: DiffEvent) => void> = [];
    let operationSeq = 0;
    const backend = {
        diff: {
            open: async (
                _repoId: number,
                _request: DiffRequest | undefined,
                onEvent: (event: DiffEvent) => void
            ): Promise<Result<number>> => {
                listeners.push(onEvent);
                return { ok: true, value: ++operationSeq };
            },
            readRange: async (): Promise<Result<RangeResult>> => ({
                ok: true,
                value: {
                    rows: [],
                    nextCursor: 0,
                    hasMore: false,
                    knownTotalRows: 0,
                    complete: true,
                },
            }),
            cancel: async () => ({ ok: true, value: true }),
        },
    } as unknown as BackendClient;

    return {
        backend,
        completeLastStream: () => {
            const listener = listeners.at(-1);
            if (!listener) return;
            const operationId = operationSeq;
            listener({
                event: "started",
                operationId,
                snapshotId: 1,
                generation: 1,
                sections: [
                    {
                        sectionId: 0,
                        path: "a.txt",
                        kind: "modified",
                        binary: false,
                        complete: false,
                    },
                ],
                estimatedTotalRows: 1,
            });
            listener({
                event: "sectionLayout",
                operationId,
                sectionId: 0,
                startRow: 0,
                rowCount: 1,
            });
            listener({
                event: "chunk",
                operationId,
                sectionId: 0,
                rowStart: 0,
                rows: [{ kind: "context", content: "x" }],
            });
            listener({
                event: "completed",
                operationId,
                totalRows: 1,
                additions: 0,
                deletions: 0,
                durationMs: 0,
            });
        },
        failNextOpen: () => {
            backend.diff.open = async (): Promise<Result<number>> => ({
                ok: false,
                error: Object.assign(new Error("boom"), {
                    name: "GitBackendError",
                    code: "internal",
                    retryable: false,
                }) as GitBackendError,
            });
        },
    };
}

const commitRequest = (commitId: string): DiffRequest => ({
    comparison: { commitToParent: { commit: commitId } },
});

beforeEach(() => {
    diffSessionRegistry.resetForTests();
});

describe("diff session registry", () => {
    it("reuses one controller per request identity and counts references", () => {
        const { backend } = fakeBackend();
        const first = diffSessionRegistry.acquire(backend, {
            repoId: 1,
            request: commitRequest("a"),
        });
        const second = diffSessionRegistry.acquire(backend, {
            repoId: 1,
            request: commitRequest("a"),
        });
        expect(second).toBe(first);

        const other = diffSessionRegistry.acquire(backend, {
            repoId: 1,
            request: commitRequest("b"),
        });
        expect(other).not.toBe(first);

        const otherTab = diffSessionRegistry.acquire(backend, {
            repoId: 1,
            tabId: "tab-2",
            request: commitRequest("a"),
        });
        expect(otherTab).not.toBe(first);
    });

    it("keeps released completed sessions warm for instant remounts", async () => {
        const { backend, completeLastStream } = fakeBackend();
        const controller = diffSessionRegistry.acquire(backend, {
            repoId: 2,
            request: commitRequest("warm"),
        });
        await controller.start();
        completeLastStream();
        expect(controller.store.state.status).toBe("completed");
        expect(controller.store.state.rowsBySection.size).toBeGreaterThan(0);

        diffSessionRegistry.release(controller);
        expect(controller.isDisposed()).toBe(false);

        const remount = diffSessionRegistry.acquire(backend, {
            repoId: 2,
            request: commitRequest("warm"),
        });
        expect(remount).toBe(controller);
        // Warm content survives; begin() must not reset it.
        remount.begin();
        expect(remount.store.state.status).toBe("completed");
        expect(remount.store.state.rowsBySection.size).toBeGreaterThan(0);
    });

    it("drops failed sessions as soon as the last consumer releases", async () => {
        const { backend, failNextOpen } = fakeBackend();
        failNextOpen();
        const controller = diffSessionRegistry.acquire(backend, {
            repoId: 3,
            request: commitRequest("bad"),
        });
        await controller.start().catch(() => {});
        expect(controller.store.state.status).toBe("failed");

        diffSessionRegistry.release(controller);
        expect(controller.isDisposed()).toBe(true);

        const next = diffSessionRegistry.acquire(backend, {
            repoId: 3,
            request: commitRequest("bad"),
        });
        expect(next).not.toBe(controller);
        expect(next.store.state.status).toBe("idle");
    });

    it("evicts oldest retained sessions beyond the LRU cap", async () => {
        const { backend, completeLastStream } = fakeBackend();
        const controllers = [];
        for (let i = 0; i < 10; i++) {
            const controller = diffSessionRegistry.acquire(backend, {
                repoId: 4,
                request: commitRequest(`bulk-${i}`),
            });
            controllers.push(controller);
            await controller.start();
            completeLastStream();
            // Completed sessions are retained, not disposed, on release.
            diffSessionRegistry.release(controller);
        }
        const byIndex = controllers;
        // Oldest zero-ref sessions are disposed once retention overflows.
        expect(byIndex[0].isDisposed()).toBe(true);
        expect(byIndex[1].isDisposed()).toBe(true);
        expect(byIndex[8].isDisposed()).toBe(false);
        expect(byIndex[9].isDisposed()).toBe(false);
    });

    it("marks every session of a repository stale", async () => {
        const { backend, completeLastStream } = fakeBackend();
        const a = diffSessionRegistry.acquire(backend, {
            repoId: 5,
            request: commitRequest("a"),
        });
        const b = diffSessionRegistry.acquire(backend, {
            repoId: 6,
            request: commitRequest("b"),
        });
        await Promise.all([a.start(), b.start()]);
        completeLastStream();

        diffSessionRegistry.markRepoStale(5);
        expect(a.store.state.status).toBe("stale");
        expect(b.store.state.status).toBe("completed");
    });

    it("re-acquires a fresh session when the generation advances", () => {
        const { backend } = fakeBackend();
        const first = diffSessionRegistry.acquire(backend, {
            repoId: 8,
            request: commitRequest("gen"),
            generation: 1,
        });
        const same = diffSessionRegistry.acquire(backend, {
            repoId: 8,
            request: commitRequest("gen"),
            generation: 1,
        });
        expect(same).toBe(first);

        const bumped = diffSessionRegistry.acquire(backend, {
            repoId: 8,
            request: commitRequest("gen"),
            generation: 2,
        });
        // A generation bump must not reuse the old snapshot's warm rows.
        expect(bumped).not.toBe(first);
    });

    it("prefetch starts streaming and returns a scoped releaser", async () => {
        const { backend, completeLastStream } = fakeBackend();
        const release = diffSessionRegistry.prefetch(backend, {
            repoId: 7,
            request: commitRequest("prefetched"),
        });
        await new Promise((resolve) => setTimeout(resolve, 0));
        completeLastStream();

        release();

        // The warmed session is handed to the real consumer with content.
        const view = diffSessionRegistry.acquire(backend, {
            repoId: 7,
            request: commitRequest("prefetched"),
        });
        expect(view.store.state.status).toBe("completed");
        expect(view.isDisposed()).toBe(false);
    });
});
