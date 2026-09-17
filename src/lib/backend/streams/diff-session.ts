import { beginOperation, finishOperation } from "@/stores/operation-store";

import type {
    DiffEvent,
    DiffImage,
    DiffRequest,
    DiffRow,
    RangeResult,
    SectionMeta,
    SyntaxStyle,
} from "../protocol";
import type { BackendClient } from "../transport/client";
import type { GitBackendError } from "../transport/invoke";
import type {
    EventContext,
    StreamSessionConfig,
    StreamSessionSpec,
} from "./stream-session";
import { StreamSessionController, streamError } from "./stream-session";

export interface SectionLayout {
    startRow: number;
    rowCount: number;
}

/** Sparse, chunk-indexed rows for one section: localStart -> rows. */
export type SectionRows = Map<number, Array<DiffRow | undefined>>;

export interface DiffSessionState {
    sessionId: string;
    repoId: number;
    operationId: number | null;
    status: "idle" | "running" | "completed" | "failed" | "cancelled" | "stale";
    sections: SectionMeta[];
    layoutBySection: Map<number, SectionLayout>;
    rowsBySection: Map<number, SectionRows>;
    /** Per-section resolved style tables; spans index into these. */
    stylesBySection: Map<number, SyntaxStyle[]>;
    imagesBySection: Map<number, DiffImage>;
    imageErrorsBySection: Map<number, GitBackendError>;
    estimatedTotalRows: number;
    totalRows: number | null;
    additions: number;
    deletions: number;
    error?: GitBackendError;
}

export interface DiffSessionConfig extends StreamSessionConfig {
    request?: DiffRequest;
}

function initialState(config: DiffSessionConfig): DiffSessionState {
    return {
        sessionId: config.sessionId,
        repoId: config.repoId,
        operationId: null,
        status: "idle",
        sections: [],
        layoutBySection: new Map(),
        rowsBySection: new Map(),
        stylesBySection: new Map(),
        imagesBySection: new Map(),
        imageErrorsBySection: new Map(),
        estimatedTotalRows: 0,
        totalRows: null,
        additions: 0,
        deletions: 0,
    };
}

const CHUNK_BUCKET = 256;

/** Folds section-local rows into bucketed storage; null when not laid out. */
function foldRows(
    state: DiffSessionState,
    sectionId: number,
    rowStart: number,
    rows: DiffRow[]
): DiffSessionState | null {
    if (rows.length === 0 || !state.layoutBySection.has(sectionId)) return null;
    const sectionRows =
        state.rowsBySection.get(sectionId) ??
        new Map<number, Array<DiffRow | undefined>>();
    const nextRows = new Map(sectionRows);
    let cursor = rowStart;
    for (const row of rows) {
        const bucketStart = Math.floor(cursor / CHUNK_BUCKET) * CHUNK_BUCKET;
        const offsetInBucket = cursor - bucketStart;
        let bucket = nextRows.get(bucketStart);
        if (!bucket) {
            bucket = Array.from({ length: CHUNK_BUCKET }, () => undefined);
            nextRows.set(bucketStart, bucket);
        }
        if (bucket[offsetInBucket] === undefined) {
            bucket[offsetInBucket] = row;
        }
        cursor += 1;
    }
    const rowsBySection = new Map(state.rowsBySection);
    rowsBySection.set(sectionId, nextRows);
    return { ...state, rowsBySection };
}

/** Folds a chunk's per-row span triples onto the rows so storage and
 * renderers see highlighted rows identically to inline `row.spans`. */
function withSpans(
    rows: DiffRow[],
    spansByRow: (number[] | null)[] | undefined
): DiffRow[] {
    if (!spansByRow) return rows;
    return rows.map((row, i) => {
        const spans = spansByRow[i];
        return spans && spans.length > 0 ? { ...row, spans } : row;
    });
}

class DiffStreamSpec implements StreamSessionSpec<
    DiffSessionState,
    DiffSessionConfig
> {
    initialState = initialState;

    async open(
        backend: BackendClient,
        config: DiffSessionConfig,
        dispatch: (event: DiffEvent) => void
    ) {
        const opened = await backend.diff.open(
            config.repoId,
            config.request,
            dispatch
        );
        return opened.ok
            ? { ok: true as const, operationId: opened.value }
            : { ok: false as const, error: opened.error };
    }

    async cancel(backend: BackendClient, operationId: number) {
        const result = await backend.diff.cancel(operationId);
        void result;
    }

    // Coverage is enforced per section inside applyRange; there is no
    // single absolute window worth pre-checking.
    isRangeCovered(): boolean {
        return false;
    }

    async readRange(
        backend: BackendClient,
        operationId: number,
        startRow: number,
        maxRows: number
    ) {
        const fetched = await backend.diff.readRange(
            operationId,
            startRow,
            maxRows
        );
        return fetched.ok
            ? { ok: true as const, value: fetched.value }
            : { ok: false as const, error: fetched.error };
    }

    applyRange(
        ctx: EventContext<DiffSessionState>,
        startRow: number,
        raw: unknown
    ): void {
        const result = raw as RangeResult;
        if (result.rows.length === 0) return;

        // Merge contiguous rows per section so each run becomes a single
        // queued fold (one store update per span, not per row).
        const bySection = new Map<
            number,
            { localStart: number; rows: DiffRow[] }[]
        >();
        let cursor = startRow;
        for (const rangeRow of result.rows) {
            const layout = ctx.state.state.layoutBySection.get(
                rangeRow.sectionId
            );
            const localIndex = layout ? cursor - layout.startRow : cursor;
            const groups = bySection.get(rangeRow.sectionId) ?? [];
            const last = groups.at(-1);
            if (last && last.localStart + last.rows.length === localIndex) {
                last.rows.push(rangeRow);
            } else {
                groups.push({ localStart: localIndex, rows: [rangeRow] });
            }
            bySection.set(rangeRow.sectionId, groups);
            cursor += 1;
        }

        for (const [sectionId, groups] of bySection) {
            let nextCursor = startRow;
            for (const group of groups) {
                const layout = ctx.state.state.layoutBySection.get(sectionId);
                const spanStart = nextCursor - (layout?.startRow ?? 0);
                const spanEnd = spanStart + group.rows.length;
                if (
                    !ctx.coverage.isCovered(
                        String(sectionId),
                        spanStart,
                        spanEnd
                    )
                ) {
                    ctx.queueChunk((state) =>
                        foldRows(state, sectionId, spanStart, group.rows)
                    );
                    ctx.coverage.record(String(sectionId), spanStart, spanEnd);
                }
                nextCursor += group.rows.length;
            }
        }
    }

    onEvent(ctx: EventContext<DiffSessionState>, event: DiffEvent): void {
        // A restarted session must not be clobbered by its previous
        // operation's terminal events still in flight.
        const operationId = ctx.operationId();
        if (
            operationId !== null &&
            "operationId" in event &&
            event.operationId !== operationId
        )
            return;
        switch (event.event) {
            case "started":
                ctx.state.setState((s) => ({
                    ...s,
                    status: s.status === "stale" ? s.status : "running",
                    sections: event.sections,
                    layoutBySection: new Map(),
                    rowsBySection: new Map(),
                    stylesBySection: new Map(),
                    imagesBySection: new Map(),
                    imageErrorsBySection: new Map(),
                    estimatedTotalRows: event.estimatedTotalRows,
                    totalRows: null,
                }));
                break;

            case "sectionLayout": {
                ctx.state.setState((s) => {
                    if (s.layoutBySection.has(event.sectionId)) return s;
                    const layoutBySection = new Map(s.layoutBySection);
                    layoutBySection.set(event.sectionId, {
                        startRow: event.startRow,
                        rowCount: event.rowCount,
                    });
                    return { ...s, layoutBySection };
                });
                // Chunks that arrived before their layout; applied on layout.
                ctx.flushPendingChunks();
                break;
            }

            case "chunk": {
                // The producer emits absolute row offsets; storage and range
                // reads are section-local.
                const layout = ctx.state.state.layoutBySection.get(
                    event.sectionId
                );
                const localStart =
                    event.rowStart - (layout?.startRow ?? event.rowStart);
                if (event.styles && event.styles.length > 0) {
                    ctx.state.setState((s) => {
                        const stylesBySection = new Map(s.stylesBySection);
                        stylesBySection.set(event.sectionId, [
                            ...(stylesBySection.get(event.sectionId) ?? []),
                            ...event.styles!,
                        ]);
                        return { ...s, stylesBySection };
                    });
                }
                const rows = withSpans(event.rows, event.spansByRow);
                ctx.queueChunk((state) =>
                    foldRows(
                        state,
                        event.sectionId,
                        Math.max(0, localStart),
                        rows
                    )
                );
                break;
            }

            case "layoutReady":
                ctx.state.setState((s) =>
                    s.totalRows === event.totalRows
                        ? s
                        : { ...s, totalRows: event.totalRows }
                );
                break;

            case "completed":
                ctx.finish("completed", undefined, {
                    totalRows: event.totalRows,
                    additions: event.additions,
                    deletions: event.deletions,
                });
                break;

            case "failed":
                ctx.finish("failed", streamError(event.code, event.message));
                break;

            case "cancelled":
                ctx.finish("cancelled");
                break;
        }
    }
}

export class DiffSessionController extends StreamSessionController<
    DiffSessionState,
    DiffSessionConfig
> {
    private inflightImages = new Map<number, Promise<void>>();

    constructor(backend: BackendClient, config: DiffSessionConfig) {
        super(backend, config, new DiffStreamSpec(), (phase, id, error) => {
            if (phase === "begin") {
                beginOperation({
                    operationId: id,
                    kind: "diff",
                    repoId: config.repoId,
                    tabId: config.tabId,
                });
            } else {
                finishOperation(id, phase, error);
            }
        });
    }

    /** Test helper: applies a wire chunk through the same fold path. */
    applyChunk(
        sectionId: number,
        rowStart: number,
        rows: DiffRow[],
        spansByRow?: (number[] | null)[]
    ): void {
        this.handleEvent({
            event: "chunk",
            operationId: this.operationId ?? 0,
            sectionId,
            rowStart,
            rows,
            ...(spansByRow ? { spansByRow } : {}),
        } as unknown as DiffEvent);
    }

    /** Fetches an image section once; overlapping requests are coalesced. */
    async ensureImage(sectionId: number): Promise<void> {
        const existing = this.inflightImages.get(sectionId);
        if (existing) return existing;
        if (this.isDisposed() || this.operationId === null) return;
        const promise = this.loadImage(sectionId);
        this.inflightImages.set(sectionId, promise);
        try {
            await promise;
        } finally {
            this.inflightImages.delete(sectionId);
        }
    }

    private async loadImage(sectionId: number): Promise<void> {
        if (this.isDisposed() || this.operationId === null) return;
        if (this.store.state.imagesBySection.has(sectionId)) return;
        const result = await this.backend.diff.readImage(
            this.operationId,
            sectionId
        );
        if (!result.ok) {
            this.store.setState((state) => {
                const errors = new Map(state.imageErrorsBySection);
                errors.set(sectionId, result.error);
                return { ...state, imageErrorsBySection: errors };
            });
            return;
        }
        if (!result.value) return;
        const image = result.value;
        this.store.setState((state) => {
            const images = new Map(state.imagesBySection);
            images.set(sectionId, image);
            return { ...state, imagesBySection: images };
        });
    }
}
