import { describe, expect, it } from "vitest";

import type {
    DiffEvent,
    DiffRequest,
    RangeResult,
} from "@/lib/backend/protocol";
import { DiffSessionController } from "@/lib/backend/streams/diff-session";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";

function fakeBackend(ranges: RangeResult[] = []) {
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
                value: ranges.shift() ?? emptyRange(),
            }),
            cancel: async () => ({ ok: true, value: true }),
        },
    } as unknown as BackendClient;

    return {
        backend,
        emit: (event: DiffEvent) => {
            for (const listener of listeners) listener(event);
        },
        get listenerCount() {
            return listeners.length;
        },
        /** Streams the last opened operation to a completed, single-row state. */
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
    };
}

function emptyRange(): RangeResult {
    return {
        rows: [],
        nextCursor: 0,
        hasMore: false,
        knownTotalRows: 0,
        complete: true,
    };
}

const startedEvent: DiffEvent = {
    event: "started",
    operationId: 1,
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
    estimatedTotalRows: 4,
};

describe("diff session controller", () => {
    it("applies layout before chunks and keeps positions stable", () => {
        const { backend } = fakeBackend();
        const session = new DiffSessionController(backend, {
            sessionId: "s1",
            repoId: 1,
        });

        session.handleEvent(startedEvent);
        session.handleEvent({
            event: "sectionLayout",
            operationId: 1,
            sectionId: 0,
            startRow: 0,
            rowCount: 2,
        });
        session.applyChunk(0, 0, [
            { kind: "context", content: "one" },
            { kind: "deletion", content: "two", oldLineno: 2 },
        ]);
        // Duplicate delivery of the same chunk must not duplicate rows.
        session.applyChunk(0, 0, [{ kind: "context", content: "one" }]);

        const state = session.store.state;
        expect(state.sections).toHaveLength(1);
        expect(state.layoutBySection.get(0)).toEqual({
            startRow: 0,
            rowCount: 2,
        });
        const bucket = state.rowsBySection.get(0)?.get(0);
        expect(bucket?.[0]?.content).toBe("one");
        expect(bucket?.[1]?.content).toBe("two");
    });

    it("rejects chunks for sections without layout", () => {
        const { backend } = fakeBackend();
        const session = new DiffSessionController(backend, {
            sessionId: "s2",
            repoId: 1,
        });
        session.handleEvent(startedEvent);
        session.applyChunk(5, 0, [
            { kind: "addition", content: "x", newLineno: 1 },
        ]);
        expect(session.store.state.rowsBySection.has(5)).toBe(false);
    });

    it("reaches terminal states exactly once per stream end", () => {
        const { backend } = fakeBackend();
        const session = new DiffSessionController(backend, {
            sessionId: "s3",
            repoId: 1,
        });
        session.handleEvent(startedEvent);
        expect(session.store.state.status).toBe("running");
        session.handleEvent({ event: "cancelled", operationId: 1 });

        session.handleEvent({
            event: "completed",
            operationId: 1,
            totalRows: 9,
            additions: 0,
            deletions: 0,
            durationMs: 1,
        });
        expect(session.store.state.status).toBe("cancelled");
    });

    it("marks sessions stale without clobbering terminal states", () => {
        const { backend } = fakeBackend();
        const session = new DiffSessionController(backend, {
            sessionId: "s4",
            repoId: 1,
        });
        session.markStale();
        expect(session.store.state.status).toBe("idle");

        session.handleEvent(startedEvent);
        session.markStale();
        expect(session.store.state.status).toBe("stale");
    });

    it("merges range reads into section-local buckets", async () => {
        const rangeResult: RangeResult = {
            rows: [
                {
                    sectionId: 0,
                    sectionKind: "modified",
                    path: "a.txt",
                    kind: "context",
                    content: "r1",
                },
                {
                    sectionId: 0,
                    sectionKind: "modified",
                    path: "a.txt",
                    kind: "addition",
                    newLineno: 9,
                    content: "r2",
                },
            ],
            nextCursor: 2,
            hasMore: false,
            knownTotalRows: 2,
            complete: true,
        };
        const { backend } = fakeBackend([rangeResult]);
        const session = new DiffSessionController(backend, {
            sessionId: "s5",
            repoId: 1,
        });
        session.handleEvent(startedEvent);
        session.handleEvent({
            event: "sectionLayout",
            operationId: 1,
            sectionId: 0,
            startRow: 10,
            rowCount: 2,
        });
        (
            session as unknown as {
                operationId: number | null;
            }
        ).operationId = 77;

        await session.ensureRange(10, 2);

        const bucket = session.store.state.rowsBySection.get(0)?.get(0);
        expect(bucket?.[0]?.content).toBe("r1");
        expect(bucket?.[1]?.content).toBe("r2");
    });

    it("normalizes absolute producer offsets to section-local buckets", async () => {
        const { backend } = fakeBackend();
        const session = new DiffSessionController(backend, {
            sessionId: "s-abs",
            repoId: 1,
        });
        session.handleEvent(startedEvent);
        session.handleEvent({
            event: "sectionLayout",
            operationId: 1,
            sectionId: 0,
            startRow: 0,
            rowCount: 2,
        });
        session.handleEvent({
            event: "sectionLayout",
            operationId: 1,
            sectionId: 1,
            startRow: 2,
            rowCount: 2,
        });
        // Producer streams absolute offsets across sections.
        session.handleEvent({
            event: "chunk",
            operationId: 1,
            sectionId: 0,
            rowStart: 0,
            rows: [
                { kind: "context", oldLineno: 1, content: "a" },
                { kind: "context", oldLineno: 2, content: "b" },
            ],
        });
        session.handleEvent({
            event: "chunk",
            operationId: 1,
            sectionId: 1,
            rowStart: 2,
            rows: [{ kind: "addition", newLineno: 1, content: "c" }],
        });
        // Section 0 painted synchronously; section 1 batches to the frame.
        await new Promise((resolve) => setTimeout(resolve, 0));
        const s0 = session.store.state.rowsBySection.get(0)?.get(0);
        expect(s0?.[0]?.content).toBe("a");
        expect(s0?.[1]?.content).toBe("b");
        const s1 = session.store.state.rowsBySection.get(1)?.get(0);
        expect(s1?.[0]?.content).toBe("c");
    });

    it("buffers chunks that arrive before their layout and applies on layout", () => {
        const { backend } = fakeBackend();
        const session = new DiffSessionController(backend, {
            sessionId: "s-buffer",
            repoId: 1,
        });
        session.handleEvent(startedEvent);
        session.handleEvent({
            event: "chunk",
            operationId: 1,
            sectionId: 0,
            rowStart: 0,
            rows: [
                { kind: "context", content: "early-1" },
                { kind: "context", content: "early-2" },
            ],
        });
        // No layout yet: nothing is dropped, but nothing renders either.
        expect(session.store.state.rowsBySection.has(0)).toBe(false);

        session.handleEvent({
            event: "sectionLayout",
            operationId: 1,
            sectionId: 0,
            startRow: 0,
            rowCount: 2,
        });
        const bucket = session.store.state.rowsBySection.get(0)?.get(0);
        expect(bucket?.[0]?.content).toBe("early-1");
        expect(bucket?.[1]?.content).toBe("early-2");
    });

    it("restart clears streamed content and begin re-streams a stale session", async () => {
        const handle = fakeBackend();
        const session = new DiffSessionController(handle.backend, {
            sessionId: "s-restart",
            repoId: 1,
        });
        await session.start();
        handle.completeLastStream();
        expect(session.store.state.status).toBe("completed");

        session.markStale();
        session.begin();

        // begin() restarts stale sessions instead of showing dead content.
        expect(session.store.state.status).toBe("running");
        expect(session.store.state.sections).toHaveLength(0);
        expect(session.store.state.rowsBySection.size).toBe(0);
        // A second open call was issued for the fresh stream.
        await new Promise((resolve) => setTimeout(resolve, 0));
        expect(handle.listenerCount).toBe(2);

        handle.completeLastStream();
        expect(session.store.state.status).toBe("completed");
    });

    it("ignores terminal events from a superseded operation after restart", async () => {
        const { backend } = fakeBackend();
        const session = new DiffSessionController(backend, {
            sessionId: "s-superseded",
            repoId: 1,
        });
        await session.start();
        session.restart();
        session.begin();
        await new Promise((resolve) => setTimeout(resolve, 0));

        // The first operation's cancellation arrives late; it must not
        // clobber the restarted stream.
        session.handleEvent({ event: "cancelled", operationId: 1 });
        expect(session.store.state.status).toBe("running");
    });
});
