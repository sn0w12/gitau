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

export function checkRunsQuery(
    deps: GithubPullsDeps,
    owner: string | null,
    repo: string | null,
    sha: string | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: githubKeys.checkRuns(owner ?? "", repo ?? "", sha ?? ""),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.listCheckRuns(
                    owner ?? "",
                    repo ?? "",
                    sha ?? ""
                )
            ),
        staleTime: 15_000,
        retry: false,
        enabled: enabled && owner != null && repo != null && !!sha,
    });
}

/** A single check run's output, fetched only when its results are opened. */
export function checkRunQuery(
    deps: GithubPullsDeps,
    owner: string | null,
    repo: string | null,
    checkRunId: number | null
) {
    return queryOptions({
        queryKey: githubKeys.checkRun(owner ?? "", repo ?? "", checkRunId ?? 0),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.getCheckRun(
                    owner ?? "",
                    repo ?? "",
                    checkRunId ?? 0
                )
            ),
        staleTime: 60_000,
        retry: false,
        enabled: owner != null && repo != null && checkRunId != null,
    });
}

/** A run's job log, split per step. Fetched when the log is first opened,
 * since it is the largest payload in the dialog. */
export function checkRunLogQuery(
    deps: GithubPullsDeps,
    owner: string | null,
    repo: string | null,
    checkRunId: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: githubKeys.checkRunLog(
            owner ?? "",
            repo ?? "",
            checkRunId ?? 0
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.getCheckRunLog(
                    owner ?? "",
                    repo ?? "",
                    checkRunId ?? 0
                )
            ),
        staleTime: 60_000,
        retry: false,
        enabled: enabled && owner != null && repo != null && checkRunId != null,
    });
}

export function workflowRunsQuery(
    deps: GithubPullsDeps,
    owner: string | null,
    repo: string | null,
    sha: string | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: githubKeys.workflowRuns(owner ?? "", repo ?? "", sha ?? ""),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.listWorkflowRuns(
                    owner ?? "",
                    repo ?? "",
                    sha ?? ""
                )
            ),
        staleTime: 15_000,
        retry: false,
        enabled: enabled && owner != null && repo != null && !!sha,
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

/** Commits on the pull request's head branch, oldest first, interleaved into
 * the conversation by the timeline builder. */
export function pullCommitsQuery(
    deps: GithubPullsDeps,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: githubKeys.pullCommits(owner ?? "", repo ?? "", number ?? 0),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.listPullCommits(
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
