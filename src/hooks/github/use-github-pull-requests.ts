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
    infiniteRepoPullsQuery,
    pullRequestQuery,
    pullReviewCommentsQuery,
    pullReviewsQuery,
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
 * One pull request plus its conversation, timeline events, submitted
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
    const reviews = useQuery(
        pullReviewsQuery({ backend }, owner, repo, number, enabled)
    );
    const reviewComments = useQuery(
        pullReviewCommentsQuery({ backend }, owner, repo, number, enabled)
    );
    return { detail, comments, events, reviews, reviewComments };
}

type PullInput = { owner: string; repo: string; number: number };

/** Invalidates the whole pull request scope: the detail, its reviews, and
 * the repo's pull request list, which the merged state changes. */
function invalidatePull(queryClient: QueryClient, input: PullInput): void {
    void queryClient.invalidateQueries({ queryKey: ["github", "pulls"] });
    void queryClient.invalidateQueries({
        queryKey: githubKeys.pullRequest(input.owner, input.repo, input.number),
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
