import {
    useInfiniteQuery,
    useMutation,
    useQuery,
    type QueryClient,
} from "@tanstack/react-query";

import { useAppServices } from "@/contexts/services-context";
import type {
    MergePullRequestBody,
    UpdatePullRequestBody,
} from "@/lib/backend/protocol";
import {
    issueCommentsQuery,
    issueEventsQuery,
    type IssueStateFilter,
} from "@/lib/backend/queries/github-issues-queries";
import {
    checkRunLogQuery,
    checkRunQuery,
    checkRunsQuery,
    commitStatusesQuery,
    infiniteRepoPullsQuery,
    infiniteSearchPullRequestsQuery,
    pullCommitsQuery,
    pullRequestQuery,
    pullReviewCommentsQuery,
    pullReviewsQuery,
    workflowRunsQuery,
} from "@/lib/backend/queries/github-pulls-queries";
import { githubKeys } from "@/lib/backend/queries/query-keys";
import { expectOk } from "@/lib/backend/transport/result";

import { useGithubAccount } from "./use-github-account";
import { useGithubCoords } from "./use-github-issues";

/** Pull requests of the repository open in `repoId`, filtered by state,
 * paginated. */
export function useRepoPullRequests(
    repoId: number | undefined,
    state: IssueStateFilter,
    labels: string[] = []
) {
    const { backend } = useAppServices();
    const account = useGithubAccount();
    const { coords } = useGithubCoords(repoId);
    const query = useInfiniteQuery(
        infiniteRepoPullsQuery(
            { backend },
            coords?.owner ?? null,
            coords?.repo ?? null,
            state,
            labels,
            account.data != null
        )
    );
    const pulls = (query.data?.pages ?? []).flatMap((page) => page.items);
    return { ...query, pulls, coords };
}

/**
 * Pull requests across all of GitHub matching `is:pr involves:@me
 * sort:updated-desc`, paginated. This is what the standalone pull requests
 * page lists; the repository-scoped `useRepoPullRequests` is the repo tab.
 */
export function useGithubSearchPullRequests() {
    const { backend } = useAppServices();
    const account = useGithubAccount();
    const login = account.data?.login ?? null;
    const query = useInfiniteQuery(
        infiniteSearchPullRequestsQuery(
            { backend },
            login,
            account.data != null
        )
    );
    const pulls = (query.data?.pages ?? []).flatMap((page) => page.items);
    return { ...query, pulls };
}

/**
 * One pull request plus its commits, conversation, timeline events, submitted
 * reviews, and inline diff comments. The conversation and event reads go
 * through the issue endpoints, which serve pull request numbers too.
 */
export function usePullRequest(
    owner: string | null,
    repo: string | null,
    number: number | null
) {
    const { backend } = useAppServices();
    const account = useGithubAccount();
    const enabled = account.data != null;
    const detail = useQuery(
        pullRequestQuery({ backend }, owner, repo, number, enabled)
    );
    const comments = useQuery(
        issueCommentsQuery({ backend }, owner, repo, number, enabled)
    );
    const events = useQuery(
        issueEventsQuery({ backend }, owner, repo, number, enabled)
    );
    const commits = useQuery(
        pullCommitsQuery({ backend }, owner, repo, number, enabled)
    );
    const reviews = useQuery(
        pullReviewsQuery({ backend }, owner, repo, number, enabled)
    );
    const reviewComments = useQuery(
        pullReviewCommentsQuery({ backend }, owner, repo, number, enabled)
    );
    // CI reports against a commit, so these wait for the detail to name the
    // head sha.
    const sha = detail.data?.head.sha ?? null;
    const checks = useQuery(
        checkRunsQuery({ backend }, owner, repo, sha, enabled)
    );
    const workflows = useQuery(
        workflowRunsQuery({ backend }, owner, repo, sha, enabled)
    );
    // Reporters still on the statuses API are a separate list GitHub merges
    // into the same place, so they are read from the same commit.
    const statuses = useQuery(
        commitStatusesQuery({ backend }, owner, repo, sha, enabled)
    );
    return {
        detail,
        comments,
        events,
        commits,
        reviews,
        reviewComments,
        checks,
        workflows,
        statuses,
    };
}

/** One check run's output, read when its results are opened. */
export function useCheckRun(
    owner: string | null,
    repo: string | null,
    checkRunId: number | null
) {
    const { backend } = useAppServices();
    const account = useGithubAccount();
    return useQuery(
        checkRunQuery(
            { backend },
            owner,
            repo,
            account.data == null ? null : checkRunId
        )
    );
}

/** A run's job log, split per step. Only fetched once `enabled`, so opening
 * the dialog does not pull megabytes of log text. */
export function useCheckRunLog(
    owner: string | null,
    repo: string | null,
    checkRunId: number | null,
    enabled: boolean
) {
    const { backend } = useAppServices();
    const account = useGithubAccount();
    return useQuery(
        checkRunLogQuery(
            { backend },
            owner,
            repo,
            account.data == null ? null : checkRunId,
            enabled
        )
    );
}

type PullInput = { owner: string; repo: string; number: number };

/** Invalidates the whole pull request scope: the detail, its reviews, the
 * issue-keyed conversation and timeline the pull page reads, the repo's pull
 * request list, and the search results, all of which the merged state
 * changes. */
function invalidatePull(queryClient: QueryClient, input: PullInput): void {
    void queryClient.invalidateQueries({ queryKey: ["github", "pulls"] });
    void queryClient.invalidateQueries({
        queryKey: ["github", "search-pull-requests"],
    });
    void queryClient.invalidateQueries({
        queryKey: githubKeys.pullRequest(input.owner, input.repo, input.number),
    });
    void queryClient.invalidateQueries({
        queryKey: githubKeys.issueDetail(input.owner, input.repo, input.number),
    });
    void queryClient.invalidateQueries({
        queryKey: githubKeys.pullReviews(input.owner, input.repo, input.number),
    });
    void queryClient.invalidateQueries({
        queryKey: githubKeys.pullReviewComments(
            input.owner,
            input.repo,
            input.number
        ),
    });
    void queryClient.invalidateQueries({ queryKey: ["github", "issues"] });
}

export function useMergePullRequest() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (input: PullInput & { body: MergePullRequestBody }) =>
            expectOk(
                await backend.github.mergePull(
                    input.owner,
                    input.repo,
                    input.number,
                    input.body
                )
            ),
        onSuccess: (_, input) => invalidatePull(queryClient, input),
    });
    return {
        pending: mutation.isPending,
        mergePull: (
            owner: string,
            repo: string,
            number: number,
            body: MergePullRequestBody
        ) => mutation.mutateAsync({ owner, repo, number, body }),
    };
}

export function useUpdatePullRequest() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (
            input: PullInput & { body: UpdatePullRequestBody }
        ) =>
            expectOk(
                await backend.github.updatePull(
                    input.owner,
                    input.repo,
                    input.number,
                    input.body
                )
            ),
        onSuccess: (_, input) => invalidatePull(queryClient, input),
    });
    return {
        pending: mutation.isPending,
        updatePull: (
            owner: string,
            repo: string,
            number: number,
            body: UpdatePullRequestBody
        ) => mutation.mutateAsync({ owner, repo, number, body }),
    };
}
