import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";

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

/** Full pull request detail: draft, mergeability, refs, and diff stats.
 * GitHub computes mergeability in the background, so a cached read can still
 * carry `mergeable: null`. */
export function pullRequestQuery(
    deps: GithubPullsDeps,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: githubKeys.pullRequest(owner ?? "", repo ?? "", number ?? 0),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.getPull(
                    owner ?? "",
                    repo ?? "",
                    number ?? 0
                )
            ),
        staleTime: 30_000,
        retry: false,
        enabled: enabled && owner != null && repo != null && number != null,
    });
}

export function pullReviewsQuery(
    deps: GithubPullsDeps,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: githubKeys.pullReviews(owner ?? "", repo ?? "", number ?? 0),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.listPullReviews(
                    owner ?? "",
                    repo ?? "",
                    number ?? 0
                )
            ),
        staleTime: 30_000,
        retry: false,
        enabled: enabled && owner != null && repo != null && number != null,
    });
}

export function pullReviewCommentsQuery(
    deps: GithubPullsDeps,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: githubKeys.pullReviewComments(
            owner ?? "",
            repo ?? "",
            number ?? 0
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.listPullReviewComments(
                    owner ?? "",
                    repo ?? "",
                    number ?? 0
                )
            ),
        staleTime: 30_000,
        retry: false,
        enabled: enabled && owner != null && repo != null && number != null,
    });
}
