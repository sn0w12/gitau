/** Frontend-owned shapes that do not cross IPC as typed Rust values. */

/** One resolved highlighting style covering both app themes. */
export interface SyntaxStyle {
    light: string;
    dark: string;
    b?: boolean;
    i?: boolean;
    u?: boolean;
}

/** A tab as persisted in the session document. `repoId` is process-local
 * and remapped on restore after repositories are reopened. */
export interface PersistedTab {
    tabId: string;
    title: string;
    lastResolvedHref?: string;
    repoPath?: string;
    repoId?: number;
    /** Title-badge registry key; resolved back to a component on restore. */
    titleBadge?: string;
}

/** The whole UI state, persisted by the backend to disk. The Rust side
 * round-trips the document as opaque JSON, so additive optional fields are
 * tolerated by older builds. */
export interface SessionDocument {
    version: number;
    tabs: PersistedTab[];
    activeTabId: string | null;
    /** Repo path -> epoch ms of the last completed fetch. */
    lastFetched?: Record<string, number>;
    /**
     * Explicitly added repositories, independent of open tabs; restored so
     * repos survive restarts even with no tab referencing them.
     */
    repositories?: { path: string; addedAt: number }[];
}
