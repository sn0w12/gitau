import {
    useInfiniteQuery,
    useQuery,
    useQueryClient,
} from "@tanstack/react-query";
import type { QueryKey } from "@tanstack/react-query";
import { useEffect } from "react";

import { useAppServices } from "@/contexts/services-context";
import type {
    CachedIcon,
    HistoryChartQuery,
    StatusOptions,
} from "@/lib/backend/protocol";
import type { HistoryPageQueryLike } from "@/lib/backend/queries/query-keys";
import { iconKeys } from "@/lib/backend/queries/query-keys";
import type { RareRead } from "@/lib/backend/queries/repository-queries";
import {
    commitDetailQuery,
    conflictFileQuery,
    fileAtRevisionQuery,
    historyChartQuery,
    historyPageQuery,
    infiniteHistoryPageQuery,
    listingQuery,
    operationStateQuery,
    rareReadQuery,
    remoteRepoInfoByPathQuery,
    remoteRepoInfoQuery,
    remotesByPathQuery,
    remotesQuery,
    snapshotQuery,
    statusQuery,
    stashQuery,
    worktreesQuery,
} from "@/lib/backend/queries/repository-queries";
import { expectOk } from "@/lib/backend/transport/result";

export {
    generationOf,
    invalidateAllRepositoryData,
    invalidateRepository,
} from "@/lib/backend/mutations/invalidation";
export type { InvalidationScope } from "@/lib/backend/mutations/invalidation";
export type { RareRead } from "@/lib/backend/queries/repository-queries";

/** Placeholder ids (0 / NaN from unparseable route params) never reach the
 * backend: pages mount before their repo binding resolves. */
function hasValidRepoId(repoId: number | undefined): boolean {
    return typeof repoId === "number" && Number.isInteger(repoId) && repoId > 0;
}

export function useRepositorySnapshot(repoId: number | undefined) {
    const { backend } = useAppServices();
    return useQuery({
        ...snapshotQuery({ backend }, repoId ?? 0),
        enabled: hasValidRepoId(repoId),
    });
}

export function useRepositoryStatus(
    repoId: number | undefined,
    options?: StatusOptions
) {
    const { backend } = useAppServices();
    return useQuery({
        ...statusQuery({ backend }, repoId ?? 0, options),
        enabled: hasValidRepoId(repoId),
    });
}

export function useOperationState(repoId: number | undefined) {
    const { backend } = useAppServices();
    return useQuery({
        ...operationStateQuery({ backend }, repoId ?? 0),
        enabled: hasValidRepoId(repoId),
    });
}

export function useConflictFile(
    repoId: number | undefined,
    path: string | undefined,
    stage: number
) {
    const { backend } = useAppServices();
    return useQuery({
        ...conflictFileQuery({ backend }, repoId ?? 0, path ?? "", stage),
        enabled: hasValidRepoId(repoId) && !!path,
    });
}

export function useRepoListing(repoId: number | undefined) {
    const { backend } = useAppServices();
    return useQuery({
        ...listingQuery({ backend }, repoId ?? 0),
        enabled: hasValidRepoId(repoId),
    });
}

export function useRemoteRepoInfo(repoId: number | undefined) {
    const { backend } = useAppServices();
    return useQuery({
        ...remoteRepoInfoQuery({ backend }, repoId ?? 0),
        enabled: hasValidRepoId(repoId),
    });
}

export function useRemoteRepoInfoByPath(path: string | undefined) {
    const { backend } = useAppServices();
    return useQuery({
        ...remoteRepoInfoByPathQuery({ backend }, path ?? ""),
        enabled: !!path,
    });
}

export function useRemotes(repoId: number | undefined) {
    const { backend } = useAppServices();
    return useQuery({
        ...remotesQuery({ backend }, repoId ?? 0),
        enabled: hasValidRepoId(repoId),
    });
}

export function useRemotesByPath(path: string | undefined) {
    const { backend } = useAppServices();
    return useQuery({
        ...remotesByPathQuery({ backend }, path ?? ""),
        enabled: !!path,
    });
}

export function useStashList(repoId: number | undefined) {
    const { backend } = useAppServices();
    return useQuery({
        ...stashQuery({ backend }, repoId ?? 0),
        enabled: hasValidRepoId(repoId),
    });
}

export function useWorktrees(repoId: number | undefined) {
    const { backend } = useAppServices();
    return useQuery({
        ...worktreesQuery({ backend }, repoId ?? 0),
        enabled: hasValidRepoId(repoId),
    });
}

export function useHistoryPage(
    repoId: number,
    query: HistoryPageQueryLike = {}
) {
    const { backend } = useAppServices();
    return useQuery({
        ...historyPageQuery({ backend }, repoId, query),
        enabled: hasValidRepoId(repoId),
    });
}

export function useInfiniteHistoryPage(repoId: number, search = "") {
    const { backend } = useAppServices();
    return useInfiniteQuery({
        ...infiniteHistoryPageQuery({ backend }, repoId, search),
        enabled: hasValidRepoId(repoId),
    });
}

export function useHistoryChart(
    repoId: number | undefined,
    config: HistoryChartQuery = {}
) {
    const { backend } = useAppServices();
    return useQuery({
        ...historyChartQuery({ backend }, repoId ?? 0, config),
        enabled: hasValidRepoId(repoId),
    });
}

/** Warms the first history page so the History tab opens without skeletons. */
export function usePrefetchHistoryPage(repoId: number) {
    const { backend } = useAppServices();
    const queryClient = useQueryClient();
    useEffect(() => {
        if (!Number.isInteger(repoId) || repoId <= 0) return;
        void queryClient.prefetchInfiniteQuery(
            infiniteHistoryPageQuery({ backend }, repoId, "")
        );
    }, [backend, queryClient, repoId]);
}

export function useCommitDetail(repoId: number, revision: string) {
    const { backend } = useAppServices();
    return useQuery({
        ...commitDetailQuery({ backend }, repoId, revision),
        enabled: hasValidRepoId(repoId),
    });
}

export function useFileAtRevision(
    repoId: number,
    revision: string,
    path: string
) {
    const { backend } = useAppServices();
    return useQuery({
        ...fileAtRevisionQuery({ backend }, repoId, revision, path),
        enabled: hasValidRepoId(repoId),
    });
}

/**
 * Resolves the cached owner/org icon for a remote URL. Freshness lives in
 * the backend's disk cache; this only memoizes the data URL per remote.
 */
export function useRemoteIcon(remoteUrl: string | undefined) {
    const { backend } = useAppServices();
    return useQuery({
        queryKey: iconKeys.byRemote(remoteUrl ?? ""),
        enabled: remoteUrl != null,
        staleTime: 10 * 60_000,
        gcTime: 60 * 60_000,
        retry: false,
        refetchOnWindowFocus: false,
        queryFn: async (): Promise<CachedIcon> =>
            expectOk(await backend.icons.resolve(remoteUrl as string)),
    });
}

/**
 * The repo page mount in one call: snapshot plus status plus history
 * prefetch, with the snapshot generation for optimistic writes.
 */
export function useRepository(
    repoId: number | undefined,
    options?: StatusOptions
) {
    const snapshot = useRepositorySnapshot(repoId);
    const status = useRepositoryStatus(repoId, options);
    usePrefetchHistoryPage(repoId ?? 0);
    return { snapshot, status, generation: snapshot.data?.generation };
}

/**
 * Single-shot rare reads through one door. History stays on its
 * dedicated hooks: its cache shape is infinite, not single-shot.
 */
export function useRepositoryRare(repoId: number | undefined, sel: RareRead) {
    const { backend } = useAppServices();
    const selected = rareReadQuery(
        { backend },
        repoId ?? 0,
        sel
    ) as unknown as {
        queryKey: QueryKey;
        queryFn: () => Promise<unknown>;
        staleTime: number;
        gcTime: number;
    };
    return useQuery({
        queryKey: selected.queryKey,
        queryFn: selected.queryFn,
        staleTime: selected.staleTime,
        gcTime: selected.gcTime,
        enabled: hasValidRepoId(repoId),
    });
}
