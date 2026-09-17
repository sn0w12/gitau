import { createStore } from "@tanstack/store";
import type { Store } from "@tanstack/store";

import type { CloneEvent, CloneProgress } from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { GitBackendError } from "@/lib/backend/transport/invoke";
import { beginOperation, finishOperation } from "@/stores/operation-store";

export interface CloneSessionState {
    status: "idle" | "running" | "completed" | "failed" | "cancelled";
    operationId: number | null;
    url: string;
    destination: string;
    progress: CloneProgress | null;
    error?: GitBackendError;
}

export interface CloneStartInput {
    url: string;
    destination: string;
}

export type CloneOutcome =
    | { status: "completed"; repoPath: string }
    | { status: "failed"; error: GitBackendError }
    | { status: "cancelled" };

function initialState(): CloneSessionState {
    return {
        status: "idle",
        operationId: null,
        url: "",
        destination: "",
        progress: null,
    };
}

function toGitError(code: string, message: string): GitBackendError {
    return Object.assign(new Error(message), {
        name: "GitBackendError",
        code,
        retryable: false,
    }) as GitBackendError;
}

/**
 * Drives one clone to completion: tracks progress for the UI, registers the
 * backend operation so `git_cancel_operation` can reach it, and resolves
 * exactly once with a terminal outcome.
 */
export class CloneSessionController {
    readonly store: Store<CloneSessionState>;

    private operationId: number | null = null;
    private terminal: ((outcome: CloneOutcome) => void) | null = null;
    /** First terminal result wins; later duplicates are ignored. */
    private settledOutcome: CloneOutcome | null = null;

    constructor(private backend: BackendClient) {
        this.store = createStore<CloneSessionState>(initialState());
    }

    /**
     * Returns to a pristine idle state, for when the owning dialog reopens.
     * A pending start() resolves as cancelled so its caller never hangs.
     */
    reset(): void {
        const pending = this.terminal;
        this.terminal = null;
        this.settledOutcome = null;
        this.operationId = null;
        this.store.setState(() => initialState());
        pending?.({ status: "cancelled" });
    }

    /** Starts the clone and resolves with its terminal outcome. */
    async start(input: CloneStartInput): Promise<CloneOutcome> {
        const state = this.store.state;
        if (state.status === "running" || state.status === "completed") {
            return { status: "cancelled" };
        }

        this.settledOutcome = null;
        this.operationId = null;
        this.store.setState(() => ({
            ...initialState(),
            status: "running",
            url: input.url,
            destination: input.destination,
        }));

        const started = await this.backend.remotes.clone(input, (event) =>
            this.handleEvent(event)
        );

        if (!started.ok) {
            return this.settle({
                status: "failed",
                error: started.error,
            });
        }

        this.operationId = started.value;
        beginOperation({
            operationId: started.value,
            kind: "clone",
        });
        this.store.setState((s) =>
            s.operationId === started.value
                ? s
                : { ...s, operationId: started.value }
        );

        // A terminal event may have raced past the invoke resolution.
        if (this.settledOutcome) return this.settledOutcome;
        return new Promise<CloneOutcome>((resolve) => {
            this.terminal = resolve;
        });
    }
    handleEvent(event: CloneEvent): void {
        switch (event.event) {
            case "progress":
                this.store.setState((s) => {
                    if (s.status !== "running") return s;
                    const sample = {
                        phase: event.phase,
                        progress: event.progress,
                        objectsReceived: event.objectsReceived,
                        objectsTotal: event.objectsTotal,
                        receivedBytes: event.receivedBytes,
                    };
                    // Byte counters are monotonic per transfer; a stray
                    // smaller tick must never shrink the readout.
                    return {
                        ...s,
                        progress: {
                            ...sample,
                            receivedBytes: Math.max(
                                s.progress?.receivedBytes ?? 0,
                                sample.receivedBytes
                            ),
                        },
                    };
                });
                break;

            case "completed":
                this.settle({ status: "completed", repoPath: event.repoPath });
                break;

            case "failed":
                this.settle({
                    status: "failed",
                    error: toGitError(event.code, event.message),
                });
                break;

            case "cancelled":
                this.settle({ status: "cancelled" });
                break;
        }
    }

    /**
     * Requests cancellation of an in-flight clone. Terminal confirmation
     * still arrives as an event; a failed cancellation call is ignored the
     * same way diff sessions treat it. With no clone running this is a
     * no-op, so unmount can call it unconditionally.
     */
    async cancel(): Promise<void> {
        if (this.operationId === null) return;
        const result = await this.backend.operations.cancel(this.operationId);
        void result;
    }

    private settle(outcome: CloneOutcome): CloneOutcome {
        if (this.settledOutcome) return this.settledOutcome;
        this.settledOutcome = outcome;
        switch (outcome.status) {
            case "completed":
                finishOperation(this.operationId ?? -1, "completed");
                this.store.setState((s) => ({
                    ...s,
                    status: "completed",
                    progress: null,
                }));
                break;
            case "failed":
                finishOperation(
                    this.operationId ?? -1,
                    "failed",
                    outcome.error
                );
                this.store.setState((s) => ({
                    ...s,
                    status: "failed",
                    error: outcome.error,
                }));
                break;
            case "cancelled":
                finishOperation(this.operationId ?? -1, "cancelled");
                this.store.setState((s) => ({ ...s, status: "cancelled" }));
                break;
        }
        const resolve = this.terminal;
        this.terminal = null;
        resolve?.(outcome);
        return outcome;
    }
}
