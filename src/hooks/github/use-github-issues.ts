import {
    useInfiniteQuery,
    useMutation,
    useQueries,
    useQuery,
    type QueryClient,
} from "@tanstack/react-query";
import { useSelector } from "@tanstack/react-store";
import { useMemo } from "react";

import { useAppServices } from "@/contexts/services-context";
import type { UpdateIssueBody } from "@/lib/backend/protocol";
import {
    infiniteRepoIssuesQuery,
    issueCommentsQuery,
    issueDetailQuery,
    issueEventsQuery,
    repoPermissionsQuery,
    type IssueStateFilter,
} from "@/lib/backend/queries/github-issues-queries";
import { infiniteSearchIssuesQuery } from "@/lib/backend/queries/github-queries";
import { githubKeys } from "@/lib/backend/queries/query-keys";
import { remotesByPathQuery } from "@/lib/backend/queries/repository-queries";
import { expectOk } from "@/lib/backend/transport/result";
import { pickGithubCoords } from "@/lib/github/repo-coords";
import { repositoryStore } from "@/stores/repository-store";

import { useRemotes } from "../repositories/use-repository-queries";
import { useGithubAccount } from "./use-github-account";

/** GitHub owner/repo of a local repository, via its remotes. Null while
 * loading or when no remote points at github.com. */
export function useGithubCoords(repoId: number | undefined) {
    const remotes = useRemotes(repoId);
    const coords = remotes.data != null ? pickGithubCoords(remotes.data) : null;
    return { ...remotes, coords };
}

/** Issues of the repository open in `repoId`, filtered by state, paginated. */
export function useRepoIssues(
    repoId: number | undefined,
    state: IssueStateFilter,
    labels: string[] = []
) {
    const { backend } = useAppServices();
    const account = useGithubAccount();
    const { coords } = useGithubCoords(repoId);
    const query = useInfiniteQuery(
        infiniteRepoIssuesQuery(
            { backend },
            coords?.owner ?? null,
            coords?.repo ?? null,
            state,
            labels,
            account.data != null
        )
    );
    const issues = (query.data?.pages ?? []).flatMap((page) => page.items);
    return { ...query, issues, coords };
}

/**
 * Maps `owner/repo` (lowercased) to the durable path of every repository in
 * the app whose remotes point at github.com, open or not. The path is the
 * identity that survives restarts, so a thread in a repo nobody has clicked
 * yet still opens in-app. Warms one cached remotes read per known repo.
 */
export function useLocalRepoMap(): Map<string, string> {
    const { backend } = useAppServices();
    const knownRepos = useSelector(repositoryStore, (state) =>
        [...state.entries.values()].map((entry) => entry.path)
    );
    const remotes = useQueries({
        queries: knownRepos.map((path) => ({
            ...remotesByPathQuery({ backend }, path),
        })),
    });
    return useMemo(() => {
        const map = new Map<string, string>();
        knownRepos.forEach((path, index) => {
            const coords = pickGithubCoords(remotes[index]?.data ?? []);
            if (coords) {
                map.set(
                    `${coords.owner.toLowerCase()}/${coords.repo.toLowerCase()}`,
                    path
                );
            }
        });
        return map;
    }, [knownRepos, remotes]);
}

/** The signed-in user's access on a repository, for gating moderation. */
export function useRepoPermissions(owner: string | null, repo: string | null) {
    const { backend } = useAppServices();
    const account = useGithubAccount();
    return useQuery(
        repoPermissionsQuery({ backend }, owner, repo, account.data != null)
    );
}

/** Issue detail plus comments plus timeline events in one hook. */
export function useIssue(
    owner: string | null,
    repo: string | null,
    number: number | null
) {
    const { backend } = useAppServices();
    const account = useGithubAccount();
    const signedIn = account.data != null;
    const detail = useQuery(
        issueDetailQuery({ backend }, owner, repo, number, signedIn)
    );
    const comments = useQuery(
        issueCommentsQuery({ backend }, owner, repo, number, signedIn)
    );
    const events = useQuery(
        issueEventsQuery({ backend }, owner, repo, number, signedIn)
    );
    return { detail, comments, events };
}

function invalidateIssue(
    queryClient: QueryClient,
    owner: string,
    repo: string,
    number?: number
) {
    void queryClient.invalidateQueries({ queryKey: ["github", "issues"] });
    if (number !== undefined) {
        void queryClient.invalidateQueries({
            queryKey: githubKeys.issueDetail(owner, repo, number),
        });
        void queryClient.invalidateQueries({
            queryKey: githubKeys.issueComments(owner, repo, number),
        });
        void queryClient.invalidateQueries({
            queryKey: githubKeys.issueEvents(owner, repo, number),
        });
    }
}

export function useCreateIssueComment() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (input: {
            owner: string;
            repo: string;
            number: number;
            body: string;
        }) =>
            expectOk(
                await backend.github.createIssueComment(
                    input.owner,
                    input.repo,
                    input.number,
                    input.body
                )
            ),
        onSuccess: (_, input) =>
            invalidateIssue(queryClient, input.owner, input.repo, input.number),
    });
    return {
        pending: mutation.isPending,
        createComment: (
            owner: string,
            repo: string,
            number: number,
            body: string
        ) => mutation.mutateAsync({ owner, repo, number, body }),
    };
}

export function useUpdateIssue() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (input: {
            owner: string;
            repo: string;
            number: number;
            body: UpdateIssueBody;
        }) =>
            expectOk(
                await backend.github.updateIssue(
                    input.owner,
                    input.repo,
                    input.number,
                    input.body
                )
            ),
        onSuccess: (_, input) =>
            invalidateIssue(queryClient, input.owner, input.repo, input.number),
    });
    return {
        pending: mutation.isPending,
        updateIssue: (
            owner: string,
            repo: string,
            number: number,
            body: UpdateIssueBody
        ) => mutation.mutateAsync({ owner, repo, number, body }),
    };
}

export function useUpdateIssueComment() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (input: {
            owner: string;
            repo: string;
            commentId: number;
            number: number;
            body: string;
        }) =>
            expectOk(
                await backend.github.updateIssueComment(
                    input.owner,
                    input.repo,
                    input.commentId,
                    input.body
                )
            ),
        onSuccess: (_, input) =>
            invalidateIssue(queryClient, input.owner, input.repo, input.number),
    });
    return {
        pending: mutation.isPending,
        updateComment: (
            owner: string,
            repo: string,
            commentId: number,
            number: number,
            body: string
        ) => mutation.mutateAsync({ owner, repo, commentId, number, body }),
    };
}

export function useDeleteIssueComment() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (input: {
            owner: string;
            repo: string;
            commentId: number;
            number: number;
        }) =>
            expectOk(
                await backend.github.deleteIssueComment(
                    input.owner,
                    input.repo,
                    input.commentId
                )
            ),
        onSuccess: (_, input) =>
            invalidateIssue(queryClient, input.owner, input.repo, input.number),
    });
    return {
        pending: mutation.isPending,
        deleteComment: (
            owner: string,
            repo: string,
            commentId: number,
            number: number
        ) => mutation.mutateAsync({ owner, repo, commentId, number }),
    };
}

export function useCreateIssue() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (input: {
            owner: string;
            repo: string;
            title: string;
            body?: string;
            labels?: string[];
        }) =>
            expectOk(
                await backend.github.createIssue(
                    input.owner,
                    input.repo,
                    input.title,
                    input.body,
                    input.labels
                )
            ),
        onSuccess: (_, input) =>
            invalidateIssue(queryClient, input.owner, input.repo),
    });
    return {
        pending: mutation.isPending,
        createIssue: (
            owner: string,
            repo: string,
            title: string,
            body?: string,
            labels?: string[]
        ) => mutation.mutateAsync({ owner, repo, title, body, labels }),
    };
}

/**
 * Paginated search results for `is:issue involves:@me
 * sort:updated-desc`, newest first. Scoped by account login.
 */
export function useGithubSearchIssues() {
    const { backend } = useAppServices();
    const account = useGithubAccount();
    const login = account.data?.login ?? null;
    const query = useInfiniteQuery(
        infiniteSearchIssuesQuery({ backend }, login, account.data != null)
    );
    const issues = (query.data?.pages ?? []).flatMap((page) => page.items);
    return { ...query, issues };
}
