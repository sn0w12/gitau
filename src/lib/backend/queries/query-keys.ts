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
     * separates cached walks per query, and the page size keys the entry
     * because `skip` offsets are computed from it: a cached walk built at one
     * page size cannot be continued at another without losing commits.
     */
    infinite: (repoId: number, search = "", pageSize?: number) =>
        [
            "repository",
            repoId,
            "history",
            "infinite",
            search || "",
            pageSize,
        ] as const,
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
    query: HistoryPageQueryLike,
    pageSize?: number
) {
    return kind === "infinite"
        ? historyKeys.infinite(repoId, query.search ?? "", pageSize)
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

export const forgeKeys = {
    all: ["forge"] as const,
    providers: () => ["forge", "providers"] as const,
    connections: () => ["forge", "connections"] as const,
    account: (provider: string) => ["forge", "account", provider] as const,
    orgs: (provider: string) => ["forge", "orgs", provider] as const,
    /**
     * Scoped by provider and account login so threads cached for one
     * connection are never reused after sign-out or a different sign-in.
     */
    notifications: (provider: string, login: string) =>
        ["forge", "notifications", provider, login] as const,
    searchIssues: (provider: string, login: string) =>
        ["forge", "search-issues", provider, login] as const,
    searchPullRequests: (provider: string, login: string) =>
        ["forge", "search-pull-requests", provider, login] as const,
    issues: (
        provider: string,
        owner: string,
        repo: string,
        state: string,
        labels: string
    ) => ["forge", "issues", provider, owner, repo, state, labels] as const,
    pulls: (
        provider: string,
        owner: string,
        repo: string,
        state: string,
        labels: string
    ) => ["forge", "pulls", provider, owner, repo, state, labels] as const,
    pullRequest: (
        provider: string,
        owner: string,
        repo: string,
        number: number
    ) => ["forge", "pull", provider, owner, repo, number] as const,
    pullReviews: (
        provider: string,
        owner: string,
        repo: string,
        number: number
    ) => ["forge", "pull", provider, owner, repo, number, "reviews"] as const,
    pullCommits: (
        provider: string,
        owner: string,
        repo: string,
        number: number
    ) => ["forge", "pull", provider, owner, repo, number, "commits"] as const,
    pullReviewComments: (
        provider: string,
        owner: string,
        repo: string,
        number: number
    ) =>
        [
            "forge",
            "pull",
            provider,
            owner,
            repo,
            number,
            "review-comments",
        ] as const,
    /** Keyed by the head commit, since that is what CI reports against. */
    checkRuns: (provider: string, owner: string, repo: string, sha: string) =>
        ["forge", "commit", provider, owner, repo, sha, "checks"] as const,
    /** Same commit, same reason: reporters still on the statuses API. */
    commitStatuses: (
        provider: string,
        owner: string,
        repo: string,
        sha: string
    ) => ["forge", "commit", provider, owner, repo, sha, "statuses"] as const,
    checkRun: (
        provider: string,
        owner: string,
        repo: string,
        checkRunId: number
    ) => ["forge", "check-run", provider, owner, repo, checkRunId] as const,
    checkRunLog: (
        provider: string,
        owner: string,
        repo: string,
        checkRunId: number
    ) => ["forge", "check-run-log", provider, owner, repo, checkRunId] as const,
    workflowRuns: (
        provider: string,
        owner: string,
        repo: string,
        sha: string
    ) => ["forge", "commit", provider, owner, repo, sha, "workflows"] as const,
    issueDetail: (
        provider: string,
        owner: string,
        repo: string,
        number: number
    ) => ["forge", "issue", provider, owner, repo, number] as const,
    issueComments: (
        provider: string,
        owner: string,
        repo: string,
        number: number
    ) => ["forge", "issue", provider, owner, repo, number, "comments"] as const,
    issueEvents: (
        provider: string,
        owner: string,
        repo: string,
        number: number
    ) => ["forge", "issue", provider, owner, repo, number, "events"] as const,
    repoPermissions: (provider: string, owner: string, repo: string) =>
        ["forge", "repo", provider, owner, repo, "permissions"] as const,
};

export const templateKeys = {
    gitignore: ["templates", "gitignore"] as const,
    licenses: ["templates", "licenses"] as const,
};

export const highlightKeys = {
    snippet: (
        language: string,
        textHash: string,
        lightTheme: string,
        darkTheme: string
    ) =>
        [
            "highlight",
            "snippet",
            language,
            textHash,
            lightTheme,
            darkTheme,
        ] as const,
};

export interface HistoryPageQueryLike {
    revision?: string;
    limit?: number;
    skip?: number;
    excludeReachableFrom?: string[];
    path?: string;
    search?: string;
}
