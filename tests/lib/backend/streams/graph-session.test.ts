// @vitest-environment jsdom
import { describe, expect, it, beforeEach } from "vitest";

import type {
    GraphEvent,
    GraphRangeResult,
    GraphRow,
} from "@/lib/backend/protocol";
import { graphSessionRegistry } from "@/lib/backend/streams/graph-session-registry";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";

function row(index: number, summaryLine = `row ${index}`): GraphRow {
    return {
        index,
        id: `111111111111111111111111111111111111111${index % 10}`,
        lane: index % 2,
        edges: [],
        kind: "commit",
        summaryLine,
        authorName: "Ada",
        authorEmail: "ada@example.com",
        timeSeconds: 1_700_000_000,
        tags: [],
        refs: [],
    };
}

function fakeBackend() {
    const listeners: Array<(event: GraphEvent) => void> = [];
    let operationSeq = 0;
    const readRangeCalls: Array<[number, number]> = [];
    const backend = {
        graph: {
            open: async (
                _repoId: number,
                _query: unknown,
                onEvent: (event: GraphEvent) => void
            ): Promise<Result<number>> => {
                listeners.push(onEvent);
                return { ok: true, value: ++operationSeq };
            },
            readRange: async (
                _operationId: number,
                startRow: number,
                maxRows: number
            ): Promise<Result<GraphRangeResult>> => {
                readRangeCalls.push([startRow, maxRows]);
                const rows = Array.from({ length: maxRows }, (_, i) =>
                    row(startRow + i)
                );
                return {
                    ok: true,
                    value: {
                        rows,
                        nextCursor: startRow + rows.length,
                        hasMore: false,
                        knownTotalRows: startRow + rows.length,
                        complete: true,
                    },
                };
            },
            cancel: async () => ({ ok: true, value: true }),
        },
    } as unknown as BackendClient;

    const emit = (event: GraphEvent) => {
        const listener = listeners.at(-1);
        if (listener) listener(event);
    };
    const streamChunks = (count: number, chunkSize: number) => {
        const operationId = operationSeq;
        emit({ event: "started", operationId, snapshotId: 1, generation: 1 });
        for (let c = 0; c < count; c++) {
            const rowStart = c * chunkSize;
            emit({
                event: "chunk",
                operationId,
                rowStart,
                rows: Array.from({ length: chunkSize }, (_, i) =>
                    row(rowStart + i)
                ),
            });
        }
        emit({ event: "completed", operationId, totalRows: count * chunkSize });
    };

    return { backend, streamChunks, readRangeCalls, emit, operationSeq };
}

beforeEach(() => {
    graphSessionRegistry.resetForTests();
});

describe("graph session", () => {
    it("applies streamed chunks and marks the prefix contiguous", async () => {
        const { backend, streamChunks } = fakeBackend();
        const controller = graphSessionRegistry.acquire(backend, { repoId: 1 });
        await controller.start();
        streamChunks(2, 3);

        expect(controller.store.state.status).toBe("completed");
        expect(controller.store.state.totalRows).toBe(6);
        expect(controller.store.state.knownTotalRows).toBe(6);
        expect(controller.store.state.chunks.size).toBe(2);
    });

    it("treats the prefix as contiguous while streaming", async () => {
        const { backend, emit } = fakeBackend();
        const controller = graphSessionRegistry.acquire(backend, { repoId: 2 });
        await controller.start();
        const op = controller.store.state.operationId as number;
        emit({
            event: "started",
            operationId: op,
            snapshotId: 1,
            generation: 1,
        });
        emit({
            event: "chunk",
            operationId: op,
            rowStart: 0,
            rows: [row(0), row(1), row(2)],
        });
        // No completed yet: state must still expose the 3-row prefix.
        expect(controller.store.state.knownTotalRows).toBe(3);
    });

    it("coalesces overlapping range reads and dedupes covered spans", async () => {
        const { backend, readRangeCalls } = fakeBackend();
        const controller = graphSessionRegistry.acquire(backend, { repoId: 3 });
        await controller.start();
        // Fire twice without awaiting: the second coalesces with the first.
        const a = controller.ensureRange(0, 5);
        const b = controller.ensureRange(0, 5);
        await Promise.all([a, b]);

        expect(readRangeCalls.filter(([start]) => start === 0).length).toBe(1);
        expect(controller.store.state.knownTotalRows).toBe(5);

        // A later request for an already-covered span is skipped outright.
        await controller.ensureRange(0, 5);
        expect(readRangeCalls.filter(([start]) => start === 0).length).toBe(1);
    });

    it("marks running sessions stale without clearing content", async () => {
        const { backend, streamChunks } = fakeBackend();
        const controller = graphSessionRegistry.acquire(backend, { repoId: 4 });
        await controller.start();
        streamChunks(1, 4);
        controller.markStale();
        expect(controller.store.state.status).toBe("stale");
        expect(controller.store.state.knownTotalRows).toBe(4);
        // A stale re-begin restarts the stream rather than keeping stale.
        controller.begin();
        expect(controller.store.state.status).toBe("running");
    });
});
