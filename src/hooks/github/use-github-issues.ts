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
    issueCommentsQuery,
    issueDetailQuery,
    issueEventsQuery,
    repoIssuesQuery,
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

/** Issues of the repository open in `repoId`, filtered by state. */
export function useRepoIssues(
    repoId: number | undefined,
    state: IssueStateFilter,
    labels: string[] = []
) {
    const { backend } = useAppServices();
    const account = useGithubAccount();
    const { coords } = useGithubCoords(repoId);
    const query = useQuery(
        repoIssuesQuery(
            { backend },
            coords?.owner ?? null,
            coords?.repo ?? null,
            state,
            labels,
            account.data != null
        )
    );
    return { ...query, coords };
}

/**
 * Maps `owner/repo` (lowercased) to the session repoId of open repos whose
 * remotes point at github.com. Only bound repos map: the issue route needs
 * a session repoId. Warms one cached remotes read per open repo.
 */
export function useLocalIssueRepoMap(): Map<string, number> {
    const { backend } = useAppServices();
    const openRepos = useSelector(repositoryStore, (state) => {
        const out: { path: string; repoId: number }[] = [];
        for (const [path, entry] of state.entries) {
            if (entry.repoId !== undefined) {
                out.push({ path, repoId: entry.repoId });
            }
        }
        return out;
    });
    const remotes = useQueries({
        queries: openRepos.map((repo) => ({
            ...remotesByPathQuery({ backend }, repo.path),
        })),
    });
    return useMemo(() => {
        const map = new Map<string, number>();
        openRepos.forEach((repo, index) => {
            const coords = pickGithubCoords(remotes[index]?.data ?? []);
            if (coords) {
                map.set(
                    `${coords.owner.toLowerCase()}/${coords.repo.toLowerCase()}`,
                    repo.repoId
                );
            }
        });
        return map;
    }, [openRepos, remotes]);
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
