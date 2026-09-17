import {
    infiniteQueryOptions,
    keepPreviousData,
    queryOptions,
} from "@tanstack/react-query";

import type {
    HistoryChartQuery,
    HistoryPage,
    RepoSnapshot,
    StatusOptions,
    StatusReport,
} from "../protocol";
import type { BackendClient } from "../transport/client";
import { expectOk } from "../transport/result";
import {
    historyKeyFor,
    historyKeys,
    repoPathKeys,
    repositoryKeys,
} from "./query-keys";
import type { HistoryPageQueryLike } from "./query-keys";

export interface RepositoryQueryDeps {
    backend: BackendClient;
}

/**
 * Every queryFn unwraps the client Result via `expectOk`: react-query
 * models failure through rejection, so an Err surfaces as `query.error`
 * with full GitBackendError detail.
 */

/**
 * Snapshot is cheap and generation-bearing; treat it as fresh briefly so
 * bursts of navigation do not spam the backend, and rely on invalidation
 * events for anything newer.
 */
export function snapshotQuery(deps: RepositoryQueryDeps, repoId: number) {
    return queryOptions({
        queryKey: repositoryKeys.snapshot(repoId),
        queryFn: async () =>
            expectOk(await deps.backend.repositories.snapshot(repoId)),
        staleTime: 15_000,
        gcTime: 10 * 60_000,
    });
}

export function statusQuery(
    deps: RepositoryQueryDeps,
    repoId: number,
    options?: StatusOptions
) {
    return queryOptions({
        queryKey: repositoryKeys.status(repoId, options),
        queryFn: async () =>
            expectOk(await deps.backend.changes.status(repoId, options)),
        staleTime: 5_000,
        gcTime: 10 * 60_000,
    });
}

export function operationStateQuery(deps: RepositoryQueryDeps, repoId: number) {
    return queryOptions({
        queryKey: repositoryKeys.operation(repoId),
        queryFn: async () =>
            expectOk(await deps.backend.workflows.operationState(repoId)),
        staleTime: 5_000,
        gcTime: 10 * 60_000,
    });
}

export function conflictFileQuery(
    deps: RepositoryQueryDeps,
    repoId: number,
    path: string,
    stage: number
) {
    return queryOptions({
        queryKey: ["repository", repoId, "conflictFile", path, stage] as const,
        queryFn: async () =>
            expectOk(
                await deps.backend.workflows.conflictFile(repoId, path, stage)
            ),
        staleTime: 30_000,
        gcTime: 10 * 60_000,
    });
}

export function listingQuery(deps: RepositoryQueryDeps, repoId: number) {
    return queryOptions({
        queryKey: repositoryKeys.listing(repoId),
        queryFn: async () =>
            expectOk(await deps.backend.refs.listBranchesAndTags(repoId)),
        staleTime: 30_000,
        gcTime: 10 * 60_000,
    });
}

export function worktreesQuery(deps: RepositoryQueryDeps, repoId: number) {
    return queryOptions({
        queryKey: repositoryKeys.worktrees(repoId),
        queryFn: async () =>
            expectOk(await deps.backend.worktrees.list(repoId)),
        staleTime: 10_000,
        gcTime: 10 * 60_000,
    });
}

export function remoteRepoInfoQuery(deps: RepositoryQueryDeps, repoId: number) {
    return queryOptions({
        queryKey: repositoryKeys.remoteInfo(repoId),
        queryFn: async () =>
            expectOk(await deps.backend.remoteInfo.info(repoId)),
        // The backend persists this and revalidates at most daily; more
        // aggressive refetching only burns provider API quota.
        staleTime: 60 * 60_000,
        gcTime: 24 * 60 * 60_000,
    });
}

export function remoteRepoInfoByPathQuery(
    deps: RepositoryQueryDeps,
    path: string
) {
    return queryOptions({
        queryKey: repoPathKeys.remoteInfo(path),
        queryFn: async () =>
            expectOk(await deps.backend.remoteInfo.byPath(path)),
        staleTime: 60 * 60_000,
        gcTime: 24 * 60 * 60_000,
    });
}

export function remotesQuery(deps: RepositoryQueryDeps, repoId: number) {
    return queryOptions({
        queryKey: repositoryKeys.remotes(repoId),
        queryFn: async () => expectOk(await deps.backend.remotes.list(repoId)),
        staleTime: 60_000,
        gcTime: 10 * 60_000,
    });
}

export function remotesByPathQuery(deps: RepositoryQueryDeps, path: string) {
    return queryOptions({
        queryKey: repoPathKeys.remotes(path),
        queryFn: async () =>
            expectOk(await deps.backend.remotes.listByPath(path)),
        staleTime: 60_000,
        gcTime: 10 * 60_000,
    });
}

/**
 * Icon for one repository: the backend probes the worktree first and falls
 * back to the owner avatar. `null` means neither source had anything.
 * Lives under the repository key prefix so invalidation sweeps refresh it.
 */
export function repoIconQuery(deps: RepositoryQueryDeps, repoId: number) {
    return queryOptions({
        queryKey: repositoryKeys.icon(repoId),
        queryFn: async () => expectOk(await deps.backend.icons.byRepo(repoId)),
        staleTime: 10 * 60_000,
        gcTime: 60 * 60_000,
    });
}

export function repoIconByPathQuery(deps: RepositoryQueryDeps, path: string) {
    return queryOptions({
        queryKey: repoPathKeys.icon(path),
        queryFn: async () => expectOk(await deps.backend.icons.byPath(path)),
        staleTime: 10 * 60_000,
        gcTime: 60 * 60_000,
    });
}

export function stashQuery(deps: RepositoryQueryDeps, repoId: number) {
    return queryOptions({
        queryKey: repositoryKeys.stash(repoId),
        queryFn: async () =>
            expectOk(await deps.backend.workflows.stashList(repoId)),
        staleTime: 30_000,
        gcTime: 10 * 60_000,
    });
}

export function hooksQuery(deps: RepositoryQueryDeps, repoId: number) {
    return queryOptions({
        queryKey: repositoryKeys.hooks(repoId),
        queryFn: async () => expectOk(await deps.backend.hooks.list(repoId)),
        staleTime: 30_000,
        gcTime: 10 * 60_000,
    });
}

export const HISTORY_PAGE_SIZE = 100;

/**
 * One fetch path for both history shapes. The backend walks and caches
 * the full revision list once per (tip, exclusions), so offset paging via
 * `skip` never re-walks; pages concatenate in order.
 */
export async function fetchHistoryPage(
    deps: RepositoryQueryDeps,
    repoId: number,
    options: { limit: number; skip: number } & HistoryPageQueryLike
): Promise<HistoryPage> {
    // Passed straight through: no copy, no extra allocation on the read
    // path. The factories own the limit/skip defaults above.
    const page = await deps.backend.history.page(repoId, options);
    return expectOk(page);
}

export function historyPageQuery(
    deps: RepositoryQueryDeps,
    repoId: number,
    query: HistoryPageQueryLike = {}
) {
    return queryOptions({
        queryKey: historyKeyFor(repoId, "page", query),
        queryFn: async () =>
            fetchHistoryPage(deps, repoId, {
                limit: 100,
                skip: 0,
                ...query,
            }),
        staleTime: 30_000,
        gcTime: 10 * 60_000,
        // Pagination/filtering swaps render the previous page instead of
        // flashing skeletons; the page itself is prefetched on repo open.
        placeholderData: keepPreviousData,
    });
}

/**
 * Accumulating pages for the History list. Shares fetchHistoryPage with
 * the single page read so filter, search, and paging semantics change in
 * one place.
 */
export function infiniteHistoryPageQuery(
    deps: RepositoryQueryDeps,
    repoId: number,
    search = ""
) {
    return infiniteQueryOptions({
        queryKey: historyKeyFor(repoId, "infinite", search ? { search } : {}),
        queryFn: async ({ pageParam }) =>
            fetchHistoryPage(deps, repoId, {
                limit: HISTORY_PAGE_SIZE,
                skip: pageParam,
                ...(search ? { search } : {}),
            }),
        initialPageParam: 0,
        getNextPageParam: (lastPage, allPages) =>
            lastPage.hasMore ? allPages.length * HISTORY_PAGE_SIZE : undefined,
        staleTime: 30_000,
        gcTime: 10 * 60_000,
        // Switching a search keeps the previous list visible while the new
        // page loads instead of flashing skeletons.
        placeholderData: keepPreviousData,
    });
}

export function commitDetailQuery(
    deps: RepositoryQueryDeps,
    repoId: number,
    revision: string,
    detectRenames = true
) {
    return queryOptions({
        queryKey: historyKeys.commitDetail(repoId, revision, detectRenames),
        queryFn: async () => {
            const detail = await deps.backend.history.commitDetail(
                repoId,
                revision,
                detectRenames
            );
            return expectOk(detail);
        },
        staleTime: Infinity,
        // Commits are immutable once written; keep them around.
        gcTime: 30 * 60_000,
    });
}

/**
 * Pre-aggregated chart buckets. Immutable history means results never go
 * stale; the backend caches the aggregation and only diffs commits it has
 * not seen yet. Invalidation events still refresh after new commits.
 */
export function historyChartQuery(
    deps: RepositoryQueryDeps,
    repoId: number,
    config: HistoryChartQuery = {}
) {
    return queryOptions({
        queryKey: historyKeys.chart(repoId, config),
        queryFn: async () =>
            expectOk(await deps.backend.history.chart(repoId, config)),
        staleTime: Infinity,
        gcTime: 30 * 60_000,
    });
}

export function fileAtRevisionQuery(
    deps: RepositoryQueryDeps,
    repoId: number,
    revision: string,
    path: string
) {
    return queryOptions({
        queryKey: historyKeys.file(repoId, revision, path),
        queryFn: async () => {
            const content = await deps.backend.history.fileAtRevision(
                repoId,
                revision,
                path
            );
            return expectOk(content);
        },
        staleTime: Infinity,
        gcTime: 30 * 60_000,
    });
}

/**
 * Single-shot rare reads behind one selector. Each arm reuses its
 * factory, so tuning stays scattered per read; the hook applies the
 * uniform guard once. History stays out: its cache shape is infinite,
 * not single-shot.
 */
export type RareRead =
    | { kind: "listing" }
    | { kind: "worktrees" }
    | { kind: "remotes" }
    | { kind: "remoteInfo" }
    | { kind: "stash" }
    | { kind: "hooks" }
    | { kind: "commitDetail"; revision: string; detectRenames?: boolean }
    | { kind: "chart"; config?: HistoryChartQuery }
    | { kind: "file"; revision: string; path: string };

export function rareReadQuery(
    deps: RepositoryQueryDeps,
    repoId: number,
    sel: RareRead
) {
    switch (sel.kind) {
        case "listing":
            return listingQuery(deps, repoId);
        case "worktrees":
            return worktreesQuery(deps, repoId);
        case "remotes":
            return remotesQuery(deps, repoId);
        case "remoteInfo":
            return remoteRepoInfoQuery(deps, repoId);
        case "stash":
            return stashQuery(deps, repoId);
        case "hooks":
            return hooksQuery(deps, repoId);
        case "commitDetail":
            return commitDetailQuery(
                deps,
                repoId,
                sel.revision,
                sel.detectRenames
            );
        case "chart":
            return historyChartQuery(deps, repoId, sel.config);
        case "file":
            return fileAtRevisionQuery(deps, repoId, sel.revision, sel.path);
    }
}

export type { StatusReport, RepoSnapshot };
