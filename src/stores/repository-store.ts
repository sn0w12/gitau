import { createStore } from "@tanstack/store";

/**
 * A known repository. The path is the durable identity: added repos stay
 * listed forever and always clickable. The backend binding (`repoId`) is
 * process-local and established lazily on first use; the backend reaps
 * idle sessions, so it may come and go across the app's lifetime.
 */
export interface RepositoryEntry {
    path: string;
    addedAt: number;
    /** Ephemeral backend binding for this process; absent until opened. */
    repoId?: number;
    lastError?: string;
}

export interface RepositoryState {
    /** Keyed by the repository path, the durable identity across restarts. */
    entries: Map<string, RepositoryEntry>;
}

function createInitialRepositoryState(): RepositoryState {
    return { entries: new Map() };
}

/**
 * Owns the set of added repositories, keyed by durable path and persisted
 * through the session document. `repoId` is an ephemeral backend binding;
 * repo data itself (snapshot, status, refs) belongs to react-query/backend.
 */
export const repositoryStore = createStore<RepositoryState>(
    createInitialRepositoryState()
);

function findKeyByRepoId(
    state: RepositoryState,
    repoId: number
): string | undefined {
    for (const [key, entry] of state.entries) {
        if (entry.repoId === repoId) return key;
    }
    return undefined;
}

function patch(
    state: RepositoryState,
    path: string,
    changes: Partial<RepositoryEntry>
): RepositoryState {
    const existing = state.entries.get(path);
    if (!existing) return state;
    const next = new Map(state.entries);
    next.set(path, { ...existing, ...changes });
    return { entries: next };
}

/** Records a repo as known without opening it; deduped case-insensitively
 * (first spelling wins). Returns the canonical path. */
export function ensureRepo(path: string): string {
    const lowered = path.toLowerCase();
    for (const entry of repositoryStore.state.entries.values()) {
        if (entry.path.toLowerCase() === lowered) return entry.path;
    }
    repositoryStore.setState((state) => {
        const next = new Map(state.entries);
        next.set(path, {
            path,
            addedAt: Date.now(),
        });
        return { entries: next };
    });
    return path;
}

export function registerOpen(path: string, repoId: number) {
    const canonical = ensureRepo(path);
    repositoryStore.setState((state) =>
        patch(state, canonical, {
            repoId,
            lastError: undefined,
        })
    );
}

/** Drops a repo from the known set. Unknown paths are a no-op. */
export function removeRepo(path: string) {
    const lowered = path.toLowerCase();
    repositoryStore.setState((state) => {
        const key = [...state.entries.keys()].find(
            (key) => key.toLowerCase() === lowered
        );
        if (!key) return state;
        const next = new Map(state.entries);
        next.delete(key);
        return { entries: next };
    });
}

/** The manual repo order is the Map's insertion order. */
export function selectRepoEntries(state: RepositoryState): RepositoryEntry[] {
    return [...state.entries.values()];
}

/**
 * Sidebar/homepage order: pinned repos first (in pinned setting order),
 * then the manual order. Pinned paths that match no entry are ignored, and
 * matching is case-insensitive like everywhere else in the store.
 */
export function selectOrderedRepoEntries(
    state: RepositoryState,
    pinnedPaths: readonly string[]
): RepositoryEntry[] {
    return [
        ...selectPinnedRepoEntries(state, pinnedPaths),
        ...selectUnpinnedRepoEntries(state, pinnedPaths),
    ];
}

/** Pinned entries in pinned setting order; stale paths ignored. */
export function selectPinnedRepoEntries(
    state: RepositoryState,
    pinnedPaths: readonly string[]
): RepositoryEntry[] {
    if (pinnedPaths.length === 0) return [];
    const entries = [...state.entries.values()];
    const rank = new Map<string, number>();
    pinnedPaths.forEach((path, index) => {
        const lowered = path.toLowerCase();
        if (!rank.has(lowered)) rank.set(lowered, index);
    });
    return entries
        .filter((entry) => rank.has(entry.path.toLowerCase()))
        .sort(
            (a, b) =>
                rank.get(a.path.toLowerCase())! -
                    rank.get(b.path.toLowerCase())! ||
                entries.indexOf(a) - entries.indexOf(b)
        );
}

/** Unpinned entries in manual (Map insertion) order. */
export function selectUnpinnedRepoEntries(
    state: RepositoryState,
    pinnedPaths: readonly string[]
): RepositoryEntry[] {
    if (pinnedPaths.length === 0) return [...state.entries.values()];
    const pinned = new Set(pinnedPaths.map((path) => path.toLowerCase()));
    return [...state.entries.values()].filter(
        (entry) => !pinned.has(entry.path.toLowerCase())
    );
}

/** Case-insensitive membership check against the pinned setting. */
export function isPinnedPath(
    path: string,
    pinnedPaths: readonly string[]
): boolean {
    const lowered = path.toLowerCase();
    return pinnedPaths.some((pinned) => pinned.toLowerCase() === lowered);
}

/** Moves a repo to a new position in the shared sidebar/homepage order.
 * Without pinned paths this reorders the raw manual order. With pinned
 * paths, `toIndex` is the position inside the unpinned group: pinned drags
 * and out-of-group drops are a no-op. Pinned order lives in the pinned
 * setting, see reorderPinnedRepos. Each sortable section reports its own
 * group-local index, so callers pass `source.index` straight through. */
export function moveRepo(
    path: string,
    toIndex: number,
    pinnedPaths?: readonly string[]
) {
    const lowered = path.toLowerCase();
    repositoryStore.setState((state) => {
        if (pinnedPaths && pinnedPaths.length > 0) {
            return moveUnpinnedInState(state, lowered, toIndex, pinnedPaths);
        }
        const keys = [...state.entries.keys()];
        const from = keys.findIndex((key) => key.toLowerCase() === lowered);
        if (from === -1) return state;

        const clamped = Math.max(0, Math.min(toIndex, keys.length - 1));
        if (clamped === from) return state;

        const [moved] = keys.splice(from, 1);
        keys.splice(clamped, 0, moved);
        const next = new Map(
            keys.map((key) => [key, state.entries.get(key)!] as const)
        );
        return { entries: next };
    });
}

function moveUnpinnedInState(
    state: RepositoryState,
    lowered: string,
    toUnpinnedIndex: number,
    pinnedPaths: readonly string[]
): RepositoryState {
    const unpinned = selectUnpinnedRepoEntries(state, pinnedPaths);
    const fromUnpinned = unpinned.findIndex(
        (entry) => entry.path.toLowerCase() === lowered
    );
    if (fromUnpinned === -1) return state;
    if (toUnpinnedIndex < 0 || toUnpinnedIndex >= unpinned.length) {
        return state;
    }
    if (toUnpinnedIndex === fromUnpinned) return state;

    const reordered = [...unpinned];
    const [moved] = reordered.splice(fromUnpinned, 1);
    reordered.splice(toUnpinnedIndex, 0, moved);

    const pinned = selectPinnedRepoEntries(state, pinnedPaths);
    const keyByLower = new Map(
        [...state.entries.keys()].map((key) => [key.toLowerCase(), key])
    );
    const next = new Map(
        [...pinned, ...reordered].map((entry) => {
            const key = keyByLower.get(entry.path.toLowerCase())!;
            return [key, state.entries.get(key)!] as const;
        })
    );
    return { entries: next };
}

/** New pinned setting order for a drag inside the pinned group.
 * `toPinnedIndex` is the position inside the pinned group. Returns null
 * for unknown paths and no-ops. */
export function reorderPinnedRepos(
    pinnedPaths: readonly string[],
    state: RepositoryState,
    path: string,
    toPinnedIndex: number
): string[] | null {
    const pinned = selectPinnedRepoEntries(state, pinnedPaths);
    const lowered = path.toLowerCase();
    const fromPinned = pinned.findIndex(
        (entry) => entry.path.toLowerCase() === lowered
    );
    if (fromPinned === -1) return null;
    if (toPinnedIndex < 0 || toPinnedIndex >= pinned.length) return null;
    if (toPinnedIndex === fromPinned) return null;

    const spellingByLower = new Map(
        pinnedPaths.map((pinnedPath) => [pinnedPath.toLowerCase(), pinnedPath])
    );
    const reordered = [...pinned];
    const [moved] = reordered.splice(fromPinned, 1);
    reordered.splice(toPinnedIndex, 0, moved);

    const visible = reordered.map(
        (entry) => spellingByLower.get(entry.path.toLowerCase()) ?? entry.path
    );
    const entryLowers = new Set(
        [...state.entries.values()].map((entry) => entry.path.toLowerCase())
    );
    const stale = pinnedPaths.filter(
        (pinnedPath) => !entryLowers.has(pinnedPath.toLowerCase())
    );
    return [...visible, ...stale];
}

export function getEntryByPath(path: string): RepositoryEntry | undefined {
    const lowered = path.toLowerCase();
    for (const entry of repositoryStore.state.entries.values()) {
        if (entry.path.toLowerCase() === lowered) return entry;
    }
    return undefined;
}

export function getEntryByRepoId(
    repoId: number | undefined
): RepositoryEntry | undefined {
    if (repoId === undefined) return undefined;
    for (const entry of repositoryStore.state.entries.values()) {
        if (entry.repoId === repoId) return entry;
    }
    return undefined;
}

export function selectRepoIdByPath(
    state: RepositoryState,
    path: string | undefined
): number | undefined {
    if (!path) return undefined;
    const lowered = path.toLowerCase();
    for (const entry of state.entries.values()) {
        if (entry.path.toLowerCase() === lowered) return entry.repoId;
    }
    return undefined;
}

export function markRepositoryError(repoId: number, message: string) {
    const path = findKeyByRepoId(repositoryStore.state, repoId);
    if (!path) return;
    repositoryStore.setState((state) =>
        patch(state, path, { lastError: message })
    );
}

/** Records a failed open; the repo stays listed and clickable to retry. */
export function setEntryError(path: string, message: string) {
    const entry = getEntryByPath(path);
    if (!entry) return;
    repositoryStore.setState((state) =>
        patch(state, entry.path, { lastError: message })
    );
}

/**
 * The backend session was released (idle reaping or explicit close). The
 * repo stays listed and clickable. The next use simply opens it again.
 */
export function unbindRepo(repoId: number) {
    const path = findKeyByRepoId(repositoryStore.state, repoId);
    if (!path) return;
    repositoryStore.setState((state) =>
        patch(state, path, { repoId: undefined })
    );
}
