import type { QueryClient } from "@tanstack/query-core";

import { repositoryKeys } from "../queries/query-keys";

/**
 * Central invalidation policy: a mutation declares which logical areas it
 * touched and this refreshes the snapshot plus every affected query prefix
 * for that repository. Snapshot is always refreshed because it carries the
 * authoritative generation.
 */
export type InvalidationScope =
    | "status"
    | "refs"
    | "history"
    | "files"
    | "remotes"
    | "stash"
    | "worktrees"
    | "operation";

const SCOPES_TO_PREFIXES: Record<InvalidationScope, readonly string[]> = {
    status: ["status"],
    refs: ["listing"],
    history: ["history", "commit"],
    files: ["file"],
    remotes: ["remotes"],
    stash: ["stash"],
    worktrees: ["worktrees"],
    operation: ["operation"],
};

export async function invalidateRepository(
    queryClient: QueryClient,
    repoId: number,
    scopes: readonly InvalidationScope[]
): Promise<void> {
    await Promise.all([
        queryClient.invalidateQueries({
            queryKey: repositoryKeys.snapshot(repoId),
        }),
        ...scopes.flatMap((scope) =>
            SCOPES_TO_PREFIXES[scope].map((segment) =>
                queryClient.invalidateQueries({
                    queryKey: [...repositoryKeys.all(repoId), segment],
                })
            )
        ),
    ]);
}

/** Full sweep used when an invalidation event arrives from the backend. */
export async function invalidateAllRepositoryData(
    queryClient: QueryClient,
    repoId: number
): Promise<void> {
    await queryClient.invalidateQueries({
        queryKey: repositoryKeys.all(repoId),
        refetchType: "active",
    });
}

export function generationOf(
    queryClient: QueryClient,
    repoId: number
): number | undefined {
    return queryClient.getQueryData<{ generation: number }>(
        repositoryKeys.snapshot(repoId)
    )?.generation;
}
