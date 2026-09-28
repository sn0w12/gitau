import { infiniteQueryOptions } from "@tanstack/react-query";

import type { BackendClient } from "@/lib/backend/transport/client";
import { expectOk } from "@/lib/backend/transport/result";

import type { IssueStateFilter } from "./github-issues-queries";
import { githubKeys } from "./query-keys";

interface GithubPullsDeps {
    backend: BackendClient;
}

/** Pull requests of one GitHub repository, newest first, paginated. */
export function infiniteRepoPullsQuery(
    deps: GithubPullsDeps,
    owner: string | null,
    repo: string | null,
    state: IssueStateFilter,
    labels: string[],
    enabled: boolean
) {
    return infiniteQueryOptions({
        queryKey: githubKeys.pulls(
            owner ?? "",
            repo ?? "",
            state,
            [...labels].sort().join(",")
        ),
        queryFn: async ({ pageParam }) =>
            expectOk(
                await deps.backend.github.listPullRequests(
                    owner ?? "",
                    repo ?? "",
                    state,
                    labels,
                    pageParam
                )
            ),
        initialPageParam: 1,
        getNextPageParam: (lastPage) =>
            lastPage.hasMore ? lastPage.page + 1 : undefined,
        staleTime: 30_000,
        retry: false,
        enabled: enabled && owner != null && repo != null,
    });
}
