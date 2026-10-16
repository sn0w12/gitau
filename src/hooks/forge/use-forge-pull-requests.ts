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
} from "@/lib/backend/queries/forge-issues-queries";
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
} from "@/lib/backend/queries/forge-pulls-queries";
import { forgeKeys } from "@/lib/backend/queries/query-keys";
import { expectOk } from "@/lib/backend/transport/result";

import { useActiveProvider, useForgeAccount } from "./use-forge-account";
import { useRepoCoords } from "./use-forge-issues";

/** Pull requests of the repository open in `repoId`, filtered by state,
 * paginated. */
export function useRepoPullRequests(
    repoId: number | undefined,
    state: IssueStateFilter,
    labels: string[] = []
) {
    const { backend } = useAppServices();
    const { coords } = useRepoCoords(repoId);
    const account = useForgeAccount(coords?.provider ?? null);
    const query = useInfiniteQuery(
        infiniteRepoPullsQuery(
            { backend },
            coords?.provider ?? null,
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
 * Pull requests across the active connection matching `is:pr
 * involves:@me sort:updated-desc`, paginated. This is what the standalone
 * pull requests page lists; the repository-scoped `useRepoPullRequests` is
 * the repo tab.
 */
export function useForgeSearchPullRequests() {
    const { backend } = useAppServices();
    const provider = useActiveProvider();
    const account = useForgeAccount(provider);
    const login = account.data?.login ?? null;
    const query = useInfiniteQuery(
        infiniteSearchPullRequestsQuery(
            { backend },
            provider,
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
    provider: string | null,
    owner: string | null,
    repo: string | null,
    number: number | null
) {
    const { backend } = useAppServices();
    const enabled = provider != null;
    const detail = useQuery(
        pullRequestQuery({ backend }, provider, owner, repo, number, enabled)
    );
    const comments = useQuery(
        issueCommentsQuery({ backend }, provider, owner, repo, number, enabled)
    );
    const events = useQuery(
        issueEventsQuery({ backend }, provider, owner, repo, number, enabled)
    );
    const commits = useQuery(
        pullCommitsQuery({ backend }, provider, owner, repo, number, enabled)
    );
    const reviews = useQuery(
        pullReviewsQuery({ backend }, provider, owner, repo, number, enabled)
    );
    const reviewComments = useQuery(
        pullReviewCommentsQuery(
            { backend },
            provider,
            owner,
            repo,
            number,
            enabled
        )
    );
    // CI reports against a commit, so these wait for the detail to name the
    // head sha.
    const sha = detail.data?.head.sha ?? null;
    const checks = useQuery(
        checkRunsQuery({ backend }, provider, owner, repo, sha, enabled)
    );
    const workflows = useQuery(
        workflowRunsQuery({ backend }, provider, owner, repo, sha, enabled)
    );
    // Reporters still on the statuses API are a separate list the provider
    // merges into the same place, so they are read from the same commit.
    const statuses = useQuery(
        commitStatusesQuery({ backend }, provider, owner, repo, sha, enabled)
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
    provider: string | null,
    owner: string | null,
    repo: string | null,
    checkRunId: number | null
) {
    const { backend } = useAppServices();
    const id = provider == null ? null : checkRunId;
    return useQuery(checkRunQuery({ backend }, provider, owner, repo, id));
}

/** A run's job log, split per step. Only fetched once `enabled`, so opening
 * the dialog does not pull megabytes of log text. */
export function useCheckRunLog(
    provider: string | null,
    owner: string | null,
    repo: string | null,
    checkRunId: number | null,
    enabled: boolean
) {
    const { backend } = useAppServices();
    const id = provider == null ? null : checkRunId;
    return useQuery(
        checkRunLogQuery({ backend }, provider, owner, repo, id, enabled)
    );
}

type PullInput = {
    provider: string;
    owner: string;
    repo: string;
    number: number;
};

/** Invalidates the whole pull request scope: the detail, its reviews, the
 * issue-keyed conversation and timeline the pull page reads, the repo's pull
 * request list, and the search results, all of which the merged state
 * changes. */
function invalidatePull(queryClient: QueryClient, input: PullInput): void {
    void queryClient.invalidateQueries({ queryKey: ["forge", "pulls"] });
    void queryClient.invalidateQueries({
        queryKey: ["forge", "search-pull-requests"],
    });
    void queryClient.invalidateQueries({
        queryKey: forgeKeys.pullRequest(
            input.provider,
            input.owner,
            input.repo,
            input.number
        ),
    });
    void queryClient.invalidateQueries({
        queryKey: forgeKeys.issueDetail(
            input.provider,
            input.owner,
            input.repo,
            input.number
        ),
    });
    void queryClient.invalidateQueries({
        queryKey: forgeKeys.pullReviews(
            input.provider,
            input.owner,
            input.repo,
            input.number
        ),
    });
    void queryClient.invalidateQueries({
        queryKey: forgeKeys.pullReviewComments(
            input.provider,
            input.owner,
            input.repo,
            input.number
        ),
    });
    void queryClient.invalidateQueries({ queryKey: ["forge", "issues"] });
}

export function useMergePullRequest() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (input: PullInput & { body: MergePullRequestBody }) =>
            expectOk(
                await backend.forge.mergePull(
                    input.provider,
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
            provider: string,
            owner: string,
            repo: string,
            number: number,
            body: MergePullRequestBody
        ) => mutation.mutateAsync({ provider, owner, repo, number, body }),
    };
}

export function useUpdatePullRequest() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (
            input: PullInput & { body: UpdatePullRequestBody }
        ) =>
            expectOk(
                await backend.forge.updatePull(
                    input.provider,
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
            provider: string,
            owner: string,
            repo: string,
            number: number,
            body: UpdatePullRequestBody
        ) => mutation.mutateAsync({ provider, owner, repo, number, body }),
    };
}
