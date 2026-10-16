import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";

import type {
    CheckRun,
    CommitStatus,
    WorkflowRun,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import { expectOk } from "@/lib/backend/transport/result";

import type { IssueStateFilter } from "./forge-issues-queries";
import { forgeKeys } from "./query-keys";

interface ForgePullsDeps {
    backend: BackendClient;
}

/** Pull requests of one repository, newest first, paginated. */
export function infiniteRepoPullsQuery(
    deps: ForgePullsDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    state: IssueStateFilter,
    labels: string[],
    enabled: boolean
) {
    return infiniteQueryOptions({
        queryKey: forgeKeys.pulls(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            state,
            [...labels].sort().join(",")
        ),
        queryFn: async ({ pageParam }) =>
            expectOk(
                await deps.backend.forge.listPullRequests(
                    provider ?? "",
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
        enabled: enabled && provider != null && owner != null && repo != null,
    });
}

/**
 * Accumulating pull requests matching `is:pr involves:@me
 * sort:updated-desc`, newest first. Scoped by provider and account login so
 * one connection never reads another's cached pages.
 */
export function infiniteSearchPullRequestsQuery(
    deps: ForgePullsDeps,
    provider: string | null,
    login: string | null,
    enabled: boolean
) {
    return infiniteQueryOptions({
        queryKey: forgeKeys.searchPullRequests(provider ?? "", login ?? ""),
        queryFn: async ({ pageParam }) =>
            expectOk(
                await deps.backend.forge.searchPullRequests(
                    provider ?? "",
                    pageParam
                )
            ),
        initialPageParam: 1,
        getNextPageParam: (lastPage) =>
            lastPage.hasMore ? lastPage.page + 1 : undefined,
        staleTime: 30_000,
        gcTime: 5 * 60_000,
        refetchInterval: 60_000,
        retry: false,
        enabled: enabled && provider != null && login != null,
    });
}

/** Full pull request detail: draft, mergeability, refs, and diff stats.
 * Mergeability can be computed in the background, so a cached read can still
 * carry `mergeable: null`. */
export function pullRequestQuery(
    deps: ForgePullsDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.pullRequest(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            number ?? 0
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.getPull(
                    provider ?? "",
                    owner ?? "",
                    repo ?? "",
                    number ?? 0
                )
            ),
        staleTime: 30_000,
        retry: false,
        enabled:
            enabled &&
            provider != null &&
            owner != null &&
            repo != null &&
            number != null,
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
    deps: ForgePullsDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    sha: string | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.checkRuns(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            sha ?? ""
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.listCheckRuns(
                    provider ?? "",
                    owner ?? "",
                    repo ?? "",
                    sha ?? ""
                )
            ),
        staleTime: 15_000,
        refetchInterval: (query) =>
            pollUntilSettled(
                query.state.data,
                (run: CheckRun) => run.status === "completed"
            ),
        retry: false,
        enabled:
            enabled &&
            provider != null &&
            owner != null &&
            repo != null &&
            !!sha,
    });
}

/** Commit statuses for a commit. Polled on the same rule as check runs, since
 * a pending status is the reporter saying it has not reported yet. */
export function commitStatusesQuery(
    deps: ForgePullsDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    sha: string | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.commitStatuses(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            sha ?? ""
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.listCommitStatuses(
                    provider ?? "",
                    owner ?? "",
                    repo ?? "",
                    sha ?? ""
                )
            ),
        staleTime: 15_000,
        refetchInterval: (query) =>
            pollUntilSettled(
                query.state.data,
                (status: CommitStatus) => status.state !== "pending"
            ),
        retry: false,
        enabled:
            enabled &&
            provider != null &&
            owner != null &&
            repo != null &&
            !!sha,
    });
}

/** A single check run's output, fetched only when its results are opened. */
export function checkRunQuery(
    deps: ForgePullsDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    checkRunId: number | null
) {
    return queryOptions({
        queryKey: forgeKeys.checkRun(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            checkRunId ?? 0
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.getCheckRun(
                    provider ?? "",
                    owner ?? "",
                    repo ?? "",
                    checkRunId ?? 0
                )
            ),
        staleTime: 60_000,
        retry: false,
        enabled:
            provider != null &&
            owner != null &&
            repo != null &&
            checkRunId != null,
    });
}

/** A run's job log, split per step. Fetched when the log is first opened,
 * since it is the largest payload in the dialog. */
export function checkRunLogQuery(
    deps: ForgePullsDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    checkRunId: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.checkRunLog(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            checkRunId ?? 0
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.getCheckRunLog(
                    provider ?? "",
                    owner ?? "",
                    repo ?? "",
                    checkRunId ?? 0
                )
            ),
        staleTime: 60_000,
        retry: false,
        enabled:
            enabled &&
            provider != null &&
            owner != null &&
            repo != null &&
            checkRunId != null,
    });
}

export function workflowRunsQuery(
    deps: ForgePullsDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    sha: string | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.workflowRuns(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            sha ?? ""
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.listWorkflowRuns(
                    provider ?? "",
                    owner ?? "",
                    repo ?? "",
                    sha ?? ""
                )
            ),
        staleTime: 15_000,
        refetchInterval: (query) =>
            pollUntilSettled(
                query.state.data,
                (run: WorkflowRun) => run.status === "completed"
            ),
        retry: false,
        enabled:
            enabled &&
            provider != null &&
            owner != null &&
            repo != null &&
            !!sha,
    });
}

export function pullReviewsQuery(
    deps: ForgePullsDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.pullReviews(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            number ?? 0
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.listPullReviews(
                    provider ?? "",
                    owner ?? "",
                    repo ?? "",
                    number ?? 0
                )
            ),
        staleTime: 30_000,
        retry: false,
        enabled:
            enabled &&
            provider != null &&
            owner != null &&
            repo != null &&
            number != null,
    });
}

/** Commits on the pull request's head branch, oldest first, interleaved into
 * the conversation by the timeline builder. */
export function pullCommitsQuery(
    deps: ForgePullsDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.pullCommits(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            number ?? 0
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.listPullCommits(
                    provider ?? "",
                    owner ?? "",
                    repo ?? "",
                    number ?? 0
                )
            ),
        staleTime: 30_000,
        retry: false,
        enabled:
            enabled &&
            provider != null &&
            owner != null &&
            repo != null &&
            number != null,
    });
}

export function pullReviewCommentsQuery(
    deps: ForgePullsDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.pullReviewComments(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            number ?? 0
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.listPullReviewComments(
                    provider ?? "",
                    owner ?? "",
                    repo ?? "",
                    number ?? 0
                )
            ),
        staleTime: 30_000,
        retry: false,
        enabled:
            enabled &&
            provider != null &&
            owner != null &&
            repo != null &&
            number != null,
    });
}
