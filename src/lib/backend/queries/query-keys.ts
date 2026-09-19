import type { HistoryChartQuery, StatusOptions } from "../protocol";

export const repositoryKeys = {
    all: (repoId: number) => ["repository", repoId] as const,
    snapshot: (repoId: number) => ["repository", repoId, "snapshot"] as const,
    operation: (repoId: number) => ["repository", repoId, "operation"] as const,
    status: (repoId: number, options?: StatusOptions) =>
        ["repository", repoId, "status", options ?? {}] as const,
    listing: (repoId: number) => ["repository", repoId, "listing"] as const,
    remoteInfo: (repoId: number) =>
        ["repository", repoId, "remoteInfo"] as const,
    remotes: (repoId: number) => ["repository", repoId, "remotes"] as const,
    icon: (repoId: number) => ["repository", repoId, "icon"] as const,
    stash: (repoId: number) => ["repository", repoId, "stash"] as const,
    hooks: (repoId: number) => ["repository", repoId, "hooks"] as const,
    worktrees: (repoId: number) => ["repository", repoId, "worktrees"] as const,
};

export const historyKeys = {
    page: (repoId: number, query: HistoryPageQueryLike) =>
        ["repository", repoId, "history", query] as const,
    /**
     * One cache entry holds every accumulated page of the infinite history
     * list; still invalidated by the "history" scope prefix. The search term
     * separates cached walks per query.
     */
    infinite: (repoId: number, search = "") =>
        ["repository", repoId, "history", "infinite", search || ""] as const,
    commitDetail: (repoId: number, revision: string, detectRenames = true) =>
        ["repository", repoId, "commit", revision, detectRenames] as const,
    chart: (repoId: number, config: HistoryChartQuery = {}) =>
        ["repository", repoId, "history", "chart", config] as const,
    file: (repoId: number, revision: string, path: string) =>
        ["repository", repoId, "file", revision, path] as const,
};

export const iconKeys = {
    byRemote: (remoteUrl: string) => ["icon", "remote", remoteUrl] as const,
};

/**
 * Single owner of both history key shapes; both history factories route
 * through here with their kind. The shapes stay distinct on purpose: a
 * single page object and an infinite pages object must never share one
 * cache slot, so page reads keep the `history` prefix and accumulating
 * reads keep `history/infinite`.
 */
export function historyKeyFor(
    repoId: number,
    kind: "page" | "infinite",
    query: HistoryPageQueryLike
) {
    return kind === "infinite"
        ? historyKeys.infinite(repoId, query.search ?? "")
        : historyKeys.page(repoId, query);
}

/**
 * Metadata keyed by the durable repo path instead of an ephemeral session
 * id, so it stays alive across tab closes and restarts.
 */
export const repoPathKeys = {
    remotes: (path: string) => ["repo-path", path, "remotes"] as const,
    remoteInfo: (path: string) => ["repo-path", path, "remoteInfo"] as const,
    icon: (path: string) => ["repo-path", path, "icon"] as const,
};

export const githubKeys = {
    all: ["github"] as const,
    account: () => ["github", "account"] as const,
    orgs: () => ["github", "orgs"] as const,
    /**
     * Scoped by account login so threads cached for one account are never
     * reused after sign-out or a different sign-in.
     */
    notifications: (login: string) =>
        ["github", "notifications", login] as const,
};

export const templateKeys = {
    gitignore: ["templates", "gitignore"] as const,
    licenses: ["templates", "licenses"] as const,
};

export interface HistoryPageQueryLike {
    revision?: string;
    limit?: number;
    skip?: number;
    excludeReachableFrom?: string[];
    path?: string;
    search?: string;
}
