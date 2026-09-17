import { createStore } from "@tanstack/store";

import type { GitBackendError } from "@/lib/backend/transport/invoke";

export type OperationKind =
    | "diff"
    | "graph"
    | "fetch"
    | "push"
    | "pull"
    | "clone"
    | "mutation";

export type OperationPhase = "running" | "completed" | "failed" | "cancelled";

export interface OperationRecord {
    operationId: number;
    kind: OperationKind;
    repoId?: number;
    tabId?: string;
    phase: OperationPhase;
    startedAt: number;
    endedAt?: number;
    error?: GitBackendError;
}

export interface OperationState {
    operations: Map<number, OperationRecord>;
}

/**
 * Owns backend operation lifecycle tracking (diff/fetch/push/…). Ephemeral:
 * ids are process-local, records die with the app. Tab association is the
 * record's `tabId`; scan by it instead of keeping parallel per-tab lists.
 */
export const operationStore = createStore<OperationState>({
    operations: new Map(),
});

export function beginOperation(input: {
    operationId: number;
    kind: OperationKind;
    repoId?: number;
    tabId?: string;
}): void {
    operationStore.setState((state) => {
        const next = new Map(state.operations);
        next.set(input.operationId, {
            operationId: input.operationId,
            kind: input.kind,
            repoId: input.repoId,
            tabId: input.tabId,
            phase: "running",
            startedAt: Date.now(),
        });
        return { operations: next };
    });
}

export function finishOperation(
    operationId: number,
    phase: Extract<OperationPhase, "completed" | "failed" | "cancelled">,
    error?: GitBackendError
): void {
    operationStore.setState((state) => {
        const existing = state.operations.get(operationId);
        if (!existing || existing.phase !== "running") return state;
        const next = new Map(state.operations);
        next.set(operationId, {
            ...existing,
            phase,
            endedAt: Date.now(),
            error,
        });
        return { operations: next };
    });
}

export function forgetOperationsForTab(tabId: string): number[] {
    const ids: number[] = [];
    operationStore.setState((state) => {
        const next = new Map<number, OperationRecord>();
        for (const [id, record] of state.operations) {
            if (record.tabId === tabId) ids.push(id);
            else next.set(id, record);
        }
        return ids.length === 0 ? state : { operations: next };
    });
    return ids;
}
