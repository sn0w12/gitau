import { createStore } from "@tanstack/store";

/**
 * Fetch-recency per repo path, persisted through the session document.
 * Nothing else should duplicate these timestamps. `inFlightByPath` tracks
 * unattended background fetches so the sync button can show them.
 */
interface FetchState {
    lastFetchedByPath: Record<string, number>;
    inFlightByPath: Record<string, true>;
}

export const fetchStore = createStore<FetchState>({
    lastFetchedByPath: {},
    inFlightByPath: {},
});

export function markRepositoryFetched(
    repoPath: string,
    fetchedAtMs: number = Date.now()
): void {
    fetchStore.setState((state) => ({
        lastFetchedByPath: {
            ...state.lastFetchedByPath,
            [repoPath]: Math.max(
                fetchedAtMs,
                state.lastFetchedByPath[repoPath] ?? 0
            ),
        },
        inFlightByPath: state.inFlightByPath,
    }));
}

export function markFetchStarted(repoPath: string): void {
    fetchStore.setState((state) => ({
        lastFetchedByPath: state.lastFetchedByPath,
        inFlightByPath: { ...state.inFlightByPath, [repoPath]: true },
    }));
}

export function markFetchEnded(repoPath: string): void {
    fetchStore.setState((state) => {
        if (state.inFlightByPath[repoPath] !== true) return state;
        const next = { ...state.inFlightByPath };
        delete next[repoPath];
        return { ...state, inFlightByPath: next };
    });
}

/** Seeds the store from a restored session document. */
export function hydrateFetchTimestamps(
    lastFetched: Record<string, number> | undefined
): void {
    if (!lastFetched || Object.keys(lastFetched).length === 0) return;
    fetchStore.setState((state) => ({
        lastFetchedByPath: { ...lastFetched },
        inFlightByPath: state.inFlightByPath,
    }));
}
