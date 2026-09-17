import { beginOperation, finishOperation } from "@/stores/operation-store";

import type {
    GraphEvent,
    GraphQuery,
    GraphRangeResult,
    GraphRow,
} from "../protocol";
import type { BackendClient } from "../transport/client";
import type { GitBackendError } from "../transport/invoke";
import type {
    EventContext,
    StreamSessionConfig,
    StreamSessionSpec,
} from "./stream-session";
import { StreamSessionController, streamError } from "./stream-session";

export type { StreamSessionStatus as GraphSessionStatus } from "./stream-session";

export interface GraphSessionState {
    sessionId: string;
    repoId: number;
    operationId: number | null;
    status: "idle" | "running" | "completed" | "failed" | "cancelled" | "stale";
    /** chunkStart (absolute row index) -> rows; mirrors the wire chunks. */
    chunks: Map<number, GraphRow[]>;
    /** End of the contiguous streamed prefix; chunks arrive in order. */
    knownTotalRows: number;
    totalRows: number | null;
    error?: GitBackendError;
}

export interface GraphSessionConfig extends StreamSessionConfig {
    query?: GraphQuery;
}

function initialState(config: GraphSessionConfig): GraphSessionState {
    return {
        sessionId: config.sessionId,
        repoId: config.repoId,
        operationId: null,
        status: "idle",
        chunks: new Map(),
        knownTotalRows: 0,
        totalRows: null,
    };
}

class GraphStreamSpec implements StreamSessionSpec<
    GraphSessionState,
    GraphSessionConfig
> {
    initialState = initialState;

    async open(
        backend: BackendClient,
        config: GraphSessionConfig,
        dispatch: (event: GraphEvent) => void
    ) {
        const opened = await backend.graph.open(
            config.repoId,
            config.query,
            dispatch
        );
        return opened.ok
            ? { ok: true as const, operationId: opened.value }
            : { ok: false as const, error: opened.error };
    }

    async cancel(backend: BackendClient, operationId: number) {
        const result = await backend.graph.cancel(operationId);
        void result;
    }

    async readRange(
        backend: BackendClient,
        operationId: number,
        startRow: number,
        maxRows: number
    ) {
        const fetched = await backend.graph.readRange(
            operationId,
            startRow,
            maxRows
        );
        return fetched.ok
            ? { ok: true as const, value: fetched.value }
            : { ok: false as const, error: fetched.error };
    }

    isRangeCovered(
        ctx: EventContext<GraphSessionState>,
        startRow: number,
        maxRows: number
    ): boolean {
        return ctx.coverage.isCovered("", startRow, startRow + maxRows);
    }

    applyRange(
        ctx: EventContext<GraphSessionState>,
        startRow: number,
        raw: unknown
    ): void {
        const result = raw as GraphRangeResult;
        if (result.rows.length === 0) return;
        const end = startRow + result.rows.length;
        if (ctx.coverage.isCovered("", startRow, end)) return;
        ctx.queueChunk((state) => {
            if (state.chunks.has(startRow)) return null;
            const chunks = new Map(state.chunks);
            chunks.set(startRow, result.rows);
            return {
                ...state,
                chunks,
                knownTotalRows: Math.max(state.knownTotalRows, end),
            };
        });
        ctx.coverage.record("", startRow, end);
    }

    onEvent(ctx: EventContext<GraphSessionState>, event: GraphEvent): void {
        // A restarted session must not be clobbered by its previous
        // operation's events still in flight.
        const operationId = ctx.operationId();
        if (operationId !== null && event.operationId !== operationId) return;
        switch (event.event) {
            case "started":
                ctx.state.setState((s) => ({
                    ...s,
                    status: s.status === "stale" ? s.status : "running",
                    chunks: new Map(),
                    knownTotalRows: 0,
                    totalRows: null,
                }));
                break;

            case "chunk": {
                const rows = event.rows;
                const rowStart = event.rowStart;
                ctx.queueChunk((state) => {
                    if (state.chunks.has(rowStart)) return null;
                    const chunks = new Map(state.chunks);
                    chunks.set(rowStart, rows);
                    let knownTotalRows = state.knownTotalRows;
                    const end = rowStart + rows.length;
                    // The contiguous prefix only grows toward higher rows.
                    if (rowStart <= knownTotalRows && end > knownTotalRows)
                        knownTotalRows = end;
                    return { ...state, chunks, knownTotalRows };
                });
                break;
            }

            case "completed":
                ctx.flushPendingChunks();
                ctx.finish("completed", undefined, {
                    totalRows: event.totalRows,
                });
                break;

            case "failed":
                ctx.flushPendingChunks();
                ctx.finish("failed", streamError(event.code, event.message));
                break;

            case "cancelled":
                ctx.flushPendingChunks();
                ctx.finish("cancelled");
                break;
        }
    }
}

export class GraphSessionController extends StreamSessionController<
    GraphSessionState,
    GraphSessionConfig
> {
    constructor(backend: BackendClient, config: GraphSessionConfig) {
        super(backend, config, new GraphStreamSpec(), (phase, id, error) => {
            if (phase === "begin") {
                beginOperation({
                    operationId: id,
                    kind: "graph",
                    repoId: config.repoId,
                    tabId: config.tabId,
                });
            } else {
                finishOperation(id, phase, error);
            }
        });
    }
}
