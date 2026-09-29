import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";

import type {
    GithubCheckRun,
    GithubCommitStatus,
    GithubWorkflowRun,
} from "@/lib/backend/protocol";
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

/**
 * Accumulating pull requests matching `is:pr involves:@me
 * sort:updated-desc` across all of GitHub, newest first. Scoped by account
 * login so one account never reads another account's cached pages.
 */
export function infiniteSearchPullRequestsQuery(
    deps: GithubPullsDeps,
    login: string | null,
    enabled: boolean
) {
    return infiniteQueryOptions({
        queryKey: githubKeys.searchPullRequests(login ?? ""),
        queryFn: async ({ pageParam }) =>
            expectOk(await deps.backend.github.searchPullRequests(pageParam)),
        initialPageParam: 1,
        getNextPageParam: (lastPage) =>
            lastPage.hasMore ? lastPage.page + 1 : undefined,
        staleTime: 30_000,
        gcTime: 5 * 60_000,
        refetchInterval: 60_000,
        retry: false,
        enabled: enabled && login != null,
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

/** CI only changes while the page is open, so results are polled until every
 * one has settled. A pull request whose CI already finished stops costing
 * requests, which is the state a reader usually leaves it in. `settled` is the
 * source's own vocabulary: check runs conclude, statuses stop being pending. */
const CHECKS_POLL_MS = 10_000;

function pollUntilSettled<T>(
    items: T[] | undefined,
    settled: (item: T) => boolean
): number | false {
    if (items === undefined) return false;
    return items.some((item) => !settled(item)) ? CHECKS_POLL_MS : false;
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
        refetchInterval: (query) =>
            pollUntilSettled(
                query.state.data,
                (run: GithubCheckRun) => run.status === "completed"
            ),
        retry: false,
        enabled: enabled && owner != null && repo != null && !!sha,
    });
}

/** Commit statuses for a commit. Polled on the same rule as check runs, since
 * a pending status is the reporter saying it has not reported yet. */
export function commitStatusesQuery(
    deps: GithubPullsDeps,
    owner: string | null,
    repo: string | null,
    sha: string | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: githubKeys.commitStatuses(owner ?? "", repo ?? "", sha ?? ""),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.listCommitStatuses(
                    owner ?? "",
                    repo ?? "",
                    sha ?? ""
                )
            ),
        staleTime: 15_000,
        refetchInterval: (query) =>
            pollUntilSettled(
                query.state.data,
                (status: GithubCommitStatus) => status.state !== "pending"
            ),
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
        refetchInterval: (query) =>
            pollUntilSettled(
                query.state.data,
                (run: GithubWorkflowRun) => run.status === "completed"
            ),
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
