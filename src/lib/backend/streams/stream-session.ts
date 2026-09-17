import { createStore } from "@tanstack/store";
import type { Store } from "@tanstack/store";

import type { BackendClient } from "../transport/client";
import type { GitBackendError } from "../transport/invoke";

export type StreamSessionStatus =
    | "idle"
    | "running"
    | "completed"
    | "failed"
    | "cancelled"
    | "stale";

export interface StreamSessionIdentity {
    sessionId: string;
    repoId: number;
    operationId: number | null;
    status: StreamSessionStatus;
}

export interface StreamSessionConfig {
    sessionId: string;
    repoId: number;
    tabId?: string;
}

/** Builds the error shape failed stream events carry onto the store. */
export function streamError(code: string, message: string): GitBackendError {
    return Object.assign(new Error(message), {
        name: "GitBackendError",
        code,
        retryable: false,
    }) as GitBackendError;
}

type Span = [number, number];

export function isCovered(spans: Span[], start: number, end: number): boolean {
    return spans.some(([s, e]) => start >= s && end <= e);
}

export function mergeSpans(spans: Span[]): Span[] {
    const merged: Span[] = [];
    for (const span of spans) {
        const previousEnd =
            merged.length > 0
                ? merged[merged.length - 1][1]
                : Number.NEGATIVE_INFINITY;
        if (span[0] <= previousEnd) {
            merged[merged.length - 1][1] = Math.max(previousEnd, span[1]);
        } else {
            merged.push(span);
        }
    }
    return merged;
}

export interface StreamCoverage {
    isCovered(key: string, start: number, end: number): boolean;
    record(key: string, start: number, end: number): void;
}

export type StreamFinishPhase = "completed" | "failed" | "cancelled";

/**
 * What a spec may touch while folding wire events: the reactive store,
 * the chunk queue, range coverage, and a terminal-phase recorder.
 */
export interface EventContext<S extends StreamSessionIdentity> {
    readonly state: Store<S>;
    readonly config: StreamSessionConfig;
    readonly coverage: StreamCoverage;
    /** Current backend operation id; changes across restarts. */
    operationId(): number | null;
    /** Queues a pure state fold; flushed at most once per frame. */
    queueChunk(fold: (state: S) => S | null): void;
    /** Applies queued folds now; folds that cannot apply stay queued. */
    flushPendingChunks(): void;
    /** Records a terminal phase once; later terminal events are no-ops. */
    finish(
        phase: StreamFinishPhase,
        error?: GitBackendError,
        patch?: Partial<S>
    ): void;
}

/** Optional backend operation tracking; only the diff stream passes one. */
export type OperationReporter = (
    phase: "begin" | StreamFinishPhase,
    operationId: number,
    error?: GitBackendError
) => void;

/**
 * The kind-specific half of a streaming session: wire protocol (open,
 * cancel, readRange) and event folding into the store. The controller
 * owns lifecycle, chunk batching, and range-read coalescing.
 */
export interface StreamSessionSpec<
    S extends StreamSessionIdentity,
    C extends StreamSessionConfig,
> {
    initialState(config: C): S;
    open(
        backend: BackendClient,
        config: C,
        dispatch: (event: unknown) => void
    ): Promise<
        | { ok: true; operationId: number }
        | { ok: false; error: GitBackendError }
    >;
    cancel(backend: BackendClient, operationId: number): Promise<unknown>;
    /** Pre-fetch coverage check; conservative answers only skip reads. */
    isRangeCovered(
        ctx: EventContext<S>,
        startRow: number,
        maxRows: number
    ): boolean;
    readRange(
        backend: BackendClient,
        operationId: number,
        startRow: number,
        maxRows: number
    ): Promise<
        { ok: true; value: unknown } | { ok: false; error: GitBackendError }
    >;
    applyRange(ctx: EventContext<S>, startRow: number, result: unknown): void;
    onEvent(ctx: EventContext<S>, event: unknown): void;
}

export class StreamSessionController<
    S extends StreamSessionIdentity,
    C extends StreamSessionConfig,
> {
    readonly store: Store<S>;
    /** Public so tests can seed an operation id without a backend open. */
    operationId: number | null = null;
    private disposed = false;

    private inflightRanges = new Map<string, Promise<void>>();
    private coverage = new Map<string, Span[]>();

    private pendingChunks: Array<(state: S) => S | null> = [];
    private flushScheduled = false;
    /** First batch flushes synchronously; afterwards updates batch per frame. */
    private hasPaintedRows = false;

    constructor(
        readonly backend: BackendClient,
        readonly config: C,
        private readonly spec: StreamSessionSpec<S, C>,
        private readonly reportOperation?: OperationReporter
    ) {
        this.store = createStore<S>(spec.initialState(config));
    }

    get sessionId(): string {
        return this.config.sessionId;
    }

    isDisposed(): boolean {
        return this.disposed;
    }

    async start(): Promise<void> {
        if (this.disposed || this.store.state.status !== "idle") return;
        this.store.setState((s) => ({ ...s, status: "running" }));
        const opened = await this.spec.open(
            this.backend,
            this.config,
            (event) => this.handleEvent(event)
        );
        if (!opened.ok) {
            // open() itself failed; no stream will arrive.
            this.store.setState((s) => ({
                ...s,
                status: "failed",
                error: opened.error,
            }));
            throw opened.error;
        }
        this.operationId = opened.operationId;
        this.reportOperation?.("begin", opened.operationId);
        this.store.setState((s) => ({ ...s, operationId: opened.operationId }));
    }

    /**
     * Mount-time entry point. Warm sessions do nothing and render instantly;
     * failed/cancelled/stale sessions reset and re-stream; idle ones start.
     */
    begin(): void {
        if (this.disposed) return;
        switch (this.store.state.status) {
            case "running":
            case "completed":
                // Warm or already streaming; render what we have.
                break;
            default:
                // Failure is recorded on the store; callers never await.
                if (this.store.state.status !== "idle") this.restart();
                void this.start().catch(() => {});
                break;
        }
    }

    /** Cancels any in-flight work and resets to a fresh idle state. */
    restart(): void {
        if (this.disposed) return;
        if (this.operationId !== null) {
            this.reportOperation?.("cancelled", this.operationId);
            void this.spec.cancel(this.backend, this.operationId);
        }
        this.operationId = null;
        this.inflightRanges.clear();
        this.coverage.clear();
        this.pendingChunks = [];
        this.flushScheduled = false;
        this.hasPaintedRows = false;
        this.store.setState(() => this.spec.initialState(this.config));
    }

    handleEvent(event: unknown): void {
        if (this.disposed) return;
        this.spec.onEvent(this.eventContext(), event);
    }

    /**
     * Fetches an absolute row range once. Overlapping or duplicate requests
     * are coalesced; range-read failures never reject outward because the
     * stream's terminal state (if any) is authoritative.
     */
    async ensureRange(startRow: number, maxRows: number): Promise<void> {
        if (this.disposed || this.operationId === null) return;
        const ctx = this.eventContext();
        if (this.spec.isRangeCovered(ctx, startRow, maxRows)) return;
        const key = `${startRow}:${maxRows}`;
        const existing = this.inflightRanges.get(key);
        if (existing) return existing;

        const promise = (async () => {
            try {
                const fetched = await this.spec.readRange(
                    this.backend,
                    this.operationId as number,
                    startRow,
                    maxRows
                );
                if (!fetched.ok) throw fetched.error;
                const before = this.store.state;
                this.spec.applyRange(ctx, startRow, fetched.value);
                if (this.store.state !== before) this.hasPaintedRows = true;
            } catch {
                // Swallowed by design; see the interface note above.
            } finally {
                this.inflightRanges.delete(key);
            }
        })();

        this.inflightRanges.set(key, promise);
        return promise;
    }

    async cancel(): Promise<void> {
        if (this.operationId === null) return;
        // A failed cancellation is ignored; the terminal state also arrives
        // via the stream.
        await this.spec.cancel(this.backend, this.operationId);
    }

    /**
     * Repository changed under this session; keep the current rows warm and
     * flag them stale so a re-acquire re-streams instead of showing dead
     * content.
     */
    markStale(): void {
        this.store.setState((s) =>
            s.status === "completed" || s.status === "running"
                ? ({ ...s, status: "stale" } as S)
                : s
        );
    }

    dispose(): void {
        if (this.disposed) return;
        this.disposed = true;
        void this.cancel();
    }

    private eventContext(): EventContext<S> {
        const coverage: StreamCoverage = {
            isCovered: (key, start, end) => {
                const spans = this.coverage.get(key);
                return spans ? isCovered(spans, start, end) : false;
            },
            record: (key, start, end) => {
                const spans = this.coverage.get(key) ?? [];
                spans.push([start, end]);
                spans.sort((a, b) => a[0] - b[0]);
                this.coverage.set(key, mergeSpans(spans));
            },
        };
        return {
            state: this.store,
            operationId: () => this.operationId,
            config: this.config,
            coverage,
            queueChunk: (fold) => {
                this.pendingChunks.push(fold);
                if (!this.hasPaintedRows) {
                    this.flushPendingChunks();
                    return;
                }
                this.scheduleFlush();
            },
            flushPendingChunks: () => this.flushPendingChunks(),
            finish: (phase, error, patch) => {
                const operationId = this.operationId;
                if (operationId !== null) {
                    this.reportOperation?.(phase, operationId, error);
                }
                this.store.setState((s) =>
                    isTerminalStatus(s.status)
                        ? s
                        : ({ ...s, status: phase, error, ...patch } as S)
                );
            },
        };
    }

    private scheduleFlush(): void {
        if (this.flushScheduled || this.disposed) return;
        this.flushScheduled = true;
        const flush = () => {
            this.flushScheduled = false;
            this.flushPendingChunks();
        };
        if (typeof requestAnimationFrame === "function") {
            requestAnimationFrame(flush);
        } else {
            setTimeout(flush, 0);
        }
    }

    /** Folds all queued chunks into one store update; unapplied stay queued. */
    flushPendingChunks(): void {
        if (this.disposed || this.pendingChunks.length === 0) return;
        const pending = this.pendingChunks;
        this.pendingChunks = [];
        let current = this.store.state;
        const keep: Array<(state: S) => S | null> = [];
        for (const fold of pending) {
            const next = fold(current);
            if (next) current = next;
            else keep.push(fold);
        }
        this.pendingChunks = keep;
        if (current !== this.store.state) {
            this.hasPaintedRows = true;
            this.store.setState(() => current);
        }
    }
}

export function isTerminalStatus(status: StreamSessionStatus): boolean {
    return (
        status === "completed" ||
        status === "failed" ||
        status === "cancelled" ||
        status === "stale"
    );
}
