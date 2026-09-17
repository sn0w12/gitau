import type { DiffRequest, GraphQuery } from "../protocol";
import type { BackendClient } from "../transport/client";
import type { DiffSessionController, DiffSessionState } from "./diff-session";
import { diffSessionRegistry } from "./diff-session-registry";
import type {
    GraphSessionController,
    GraphSessionState,
} from "./graph-session";
import { graphSessionRegistry } from "./graph-session-registry";

/**
 * One narrow seam over both stream kinds. Diff and graph share registry
 * shape (repo plus tab plus request plus generation), coverage, the
 * stale-guard, the terminal path, and operation reporting; only folding
 * and range reads stay kind-specific. Callers learn these names and stop
 * fanning out to two registries by hand.
 */

export interface DiffAcquireInput {
    repoId: number;
    tabId?: string;
    request?: DiffRequest;
    generation?: number;
    /** Begin the stream on acquire, folding prefetch into one call. */
    start?: boolean;
}

export interface GraphAcquireInput {
    repoId: number;
    tabId?: string;
    query?: GraphQuery;
    generation?: number;
    /** Begin the stream on acquire, folding prefetch into one call. */
    start?: boolean;
}

export type StreamController = DiffSessionController | GraphSessionController;

export function acquireDiff(
    backend: BackendClient,
    input: DiffAcquireInput
): DiffSessionController {
    return diffSessionRegistry.acquire(backend, input);
}

export function acquireGraph(
    backend: BackendClient,
    input: GraphAcquireInput
): GraphSessionController {
    return graphSessionRegistry.acquire(backend, input);
}

export function releaseStream(controller: StreamController): void {
    diffSessionRegistry.release(controller as DiffSessionController);
    graphSessionRegistry.release(controller as GraphSessionController);
}

export function prefetchDiff(
    backend: BackendClient,
    input: Omit<DiffAcquireInput, "start" | "generation">
): () => void {
    return diffSessionRegistry.prefetch(backend, input);
}

export function prefetchGraph(
    backend: BackendClient,
    input: Omit<GraphAcquireInput, "start">
): () => void {
    const controller = acquireGraph(backend, { ...input, start: true });
    return () => graphSessionRegistry.release(controller);
}

export function markStreamsStale(repoId: number): void {
    diffSessionRegistry.markRepoStale(repoId);
    graphSessionRegistry.markRepoStale(repoId);
}

export function disposeStreamsForTab(tabId: string): void {
    diffSessionRegistry.disposeAllForTab(tabId);
    graphSessionRegistry.disposeAllForTab(tabId);
}

export function disposeStreamsForRepo(repoId: number): void {
    diffSessionRegistry.disposeAllForRepo(repoId);
    graphSessionRegistry.disposeAllForRepo(repoId);
}

/** Test helper: drops every session in both registries. */
export function resetStreamsForTests(): void {
    diffSessionRegistry.resetForTests();
    graphSessionRegistry.resetForTests();
}

export type { DiffSessionState, GraphSessionState };
