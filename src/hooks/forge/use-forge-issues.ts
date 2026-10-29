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
import type { Capabilities, UpdateIssueBody } from "@/lib/backend/protocol";
import {
    infiniteRepoIssuesQuery,
    issueCommentsQuery,
    issueDetailQuery,
    issueEventsQuery,
    repoLabelsQuery,
    repoPermissionsQuery,
    type IssueStateFilter,
} from "@/lib/backend/queries/forge-issues-queries";
import { infiniteSearchIssuesQuery } from "@/lib/backend/queries/forge-queries";
import { forgeKeys } from "@/lib/backend/queries/query-keys";
import { remotesByPathQuery } from "@/lib/backend/queries/repository-queries";
import { expectOk } from "@/lib/backend/transport/result";
import { pickRepoCoords } from "@/lib/forge/repo-coords";
import type { RepoView } from "@/lib/routing/repo-search";
import { repositoryStore } from "@/stores/repository-store";

import { useRemotes } from "../repositories/use-repository-queries";
import {
    useActiveProvider,
    useForgeAccount,
    useForgeProviders,
    capabilityAllows,
} from "./use-forge-account";

/** Provider coordinates of a local repository, via its remotes. Null while
 * loading or when no remote is claimed by a known provider. */
export function useRepoCoords(repoId: number | undefined) {
    const remotes = useRemotes(repoId);
    const providers = useForgeProviders();
    const coords =
        remotes.data != null
            ? pickRepoCoords(remotes.data, providers.data ?? [])
            : null;
    return { ...remotes, coords };
}

/**
 * What the repository's own provider can do. Repo-scoped views gate on this
 * rather than on the selected connection, so a repository on one provider is
 * judged by that provider.
 */
export function useRepoCapabilities(
    repoId: number | undefined
): Capabilities | null {
    const { coords } = useRepoCoords(repoId);
    const providers = useForgeProviders();
    if (coords == null) return null;
    return (
        providers.data?.find((entry) => entry.id === coords.provider)
            ?.capabilities ?? null
    );
}

/**
 * Which views a repository can open.
 */
export function useRepoViewAccess(
    repoId: number | undefined
): Record<RepoView, boolean> {
    const { coords, data: remotes } = useRepoCoords(repoId);
    const providers = useForgeProviders();
    const capabilities = useRepoCapabilities(repoId);
    if (coords === null) {
        const unresolved =
            remotes === undefined || providers.data === undefined;
        return {
            overview: true,
            graph: true,
            issues: unresolved,
            pulls: unresolved,
        };
    }
    return {
        overview: true,
        graph: true,
        issues: capabilityAllows(capabilities, "issues"),
        pulls: capabilityAllows(capabilities, "pullRequests"),
    };
}

/** Issues of the repository open in `repoId`, filtered by state, paginated. */
export function useRepoIssues(
    repoId: number | undefined,
    state: IssueStateFilter,
    labels: string[] = []
) {
    const { backend } = useAppServices();
    const { coords } = useRepoCoords(repoId);
    const account = useForgeAccount(coords?.provider ?? null);
    const query = useInfiniteQuery(
        infiniteRepoIssuesQuery(
            { backend },
            coords?.provider ?? null,
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
 * the app whose remotes are claimed by a known provider, open or not. The
 * path is the identity that survives restarts, so a thread in a repo nobody
 * has clicked yet still opens in-app. Warms one cached remotes read per
 * known repo.
 */
export function useLocalRepoMap(): Map<string, string> {
    const { backend } = useAppServices();
    const providers = useForgeProviders();
    const knownRepos = useSelector(repositoryStore, (state) =>
        [...state.entries.values()].map((entry) => entry.path)
    );
    const remotes = useQueries({
        queries: knownRepos.map((path) => ({
            ...remotesByPathQuery({ backend }, path),
        })),
    });
    const providerList = providers.data;
    return useMemo(() => {
        const map = new Map<string, string>();
        knownRepos.forEach((path, index) => {
            const coords = pickRepoCoords(
                remotes[index]?.data ?? [],
                providerList ?? []
            );
            if (coords) {
                map.set(
                    `${coords.owner.toLowerCase()}/${coords.repo.toLowerCase()}`,
                    path
                );
            }
        });
        return map;
    }, [knownRepos, remotes, providerList]);
}

/** The signed-in user's access on a repository, for gating moderation. */
export function useRepoPermissions(
    provider: string | null,
    owner: string | null,
    repo: string | null
) {
    const { backend } = useAppServices();
    return useQuery(
        repoPermissionsQuery(
            { backend },
            provider,
            owner,
            repo,
            provider != null
        )
    );
}

export function useRepoLabels(
    provider: string | null,
    owner: string | null,
    repo: string | null
) {
    const { backend } = useAppServices();
    return useQuery(
        repoLabelsQuery({ backend }, provider, owner, repo, provider != null)
    );
}

/** Issue detail plus comments plus timeline events in one hook. */
export function useIssue(
    provider: string | null,
    owner: string | null,
    repo: string | null,
    number: number | null
) {
    const { backend } = useAppServices();
    const enabled = provider != null;
    const detail = useQuery(
        issueDetailQuery({ backend }, provider, owner, repo, number, enabled)
    );
    const comments = useQuery(
        issueCommentsQuery({ backend }, provider, owner, repo, number, enabled)
    );
    const events = useQuery(
        issueEventsQuery({ backend }, provider, owner, repo, number, enabled)
    );
    return { detail, comments, events };
}

function invalidateIssue(
    queryClient: QueryClient,
    provider: string,
    owner: string,
    repo: string,
    number?: number
) {
    void queryClient.invalidateQueries({ queryKey: ["forge", "issues"] });
    if (number !== undefined) {
        void queryClient.invalidateQueries({
            queryKey: forgeKeys.issueDetail(provider, owner, repo, number),
        });
        void queryClient.invalidateQueries({
            queryKey: forgeKeys.issueComments(provider, owner, repo, number),
        });
        void queryClient.invalidateQueries({
            queryKey: forgeKeys.issueEvents(provider, owner, repo, number),
        });
    }
}

interface IssueCommentInput {
    provider: string;
    owner: string;
    repo: string;
    number: number;
    body: string;
}

export function useCreateIssueComment() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (input: IssueCommentInput) =>
            expectOk(
                await backend.forge.createIssueComment(
                    input.provider,
                    input.owner,
                    input.repo,
                    input.number,
                    input.body
                )
            ),
        onSuccess: (_, input) =>
            invalidateIssue(
                queryClient,
                input.provider,
                input.owner,
                input.repo,
                input.number
            ),
    });
    return {
        pending: mutation.isPending,
        createComment: (
            provider: string,
            owner: string,
            repo: string,
            number: number,
            body: string
        ) => mutation.mutateAsync({ provider, owner, repo, number, body }),
    };
}

export function useUpdateIssue() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (input: {
            provider: string;
            owner: string;
            repo: string;
            number: number;
            body: UpdateIssueBody;
        }) =>
            expectOk(
                await backend.forge.updateIssue(
                    input.provider,
                    input.owner,
                    input.repo,
                    input.number,
                    input.body
                )
            ),
        onSuccess: (_, input) =>
            invalidateIssue(
                queryClient,
                input.provider,
                input.owner,
                input.repo,
                input.number
            ),
    });
    return {
        pending: mutation.isPending,
        updateIssue: (
            provider: string,
            owner: string,
            repo: string,
            number: number,
            body: UpdateIssueBody
        ) => mutation.mutateAsync({ provider, owner, repo, number, body }),
    };
}

export function useUpdateIssueComment() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (input: {
            provider: string;
            owner: string;
            repo: string;
            commentId: number;
            number: number;
            body: string;
        }) =>
            expectOk(
                await backend.forge.updateIssueComment(
                    input.provider,
                    input.owner,
                    input.repo,
                    input.commentId,
                    input.body
                )
            ),
        onSuccess: (_, input) =>
            invalidateIssue(
                queryClient,
                input.provider,
                input.owner,
                input.repo,
                input.number
            ),
    });
    return {
        pending: mutation.isPending,
        updateComment: (
            provider: string,
            owner: string,
            repo: string,
            commentId: number,
            number: number,
            body: string
        ) =>
            mutation.mutateAsync({
                provider,
                owner,
                repo,
                commentId,
                number,
                body,
            }),
    };
}

export function useDeleteIssueComment() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (input: {
            provider: string;
            owner: string;
            repo: string;
            commentId: number;
            number: number;
        }) =>
            expectOk(
                await backend.forge.deleteIssueComment(
                    input.provider,
                    input.owner,
                    input.repo,
                    input.commentId
                )
            ),
        onSuccess: (_, input) =>
            invalidateIssue(
                queryClient,
                input.provider,
                input.owner,
                input.repo,
                input.number
            ),
    });
    return {
        pending: mutation.isPending,
        deleteComment: (
            provider: string,
            owner: string,
            repo: string,
            commentId: number,
            number: number
        ) =>
            mutation.mutateAsync({
                provider,
                owner,
                repo,
                commentId,
                number,
            }),
    };
}

export function useCreateIssue() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (input: {
            provider: string;
            owner: string;
            repo: string;
            title: string;
            body?: string;
            labels?: string[];
        }) =>
            expectOk(
                await backend.forge.createIssue(
                    input.provider,
                    input.owner,
                    input.repo,
                    input.title,
                    input.body,
                    input.labels
                )
            ),
        onSuccess: (_, input) =>
            invalidateIssue(
                queryClient,
                input.provider,
                input.owner,
                input.repo
            ),
    });
    return {
        pending: mutation.isPending,
        createIssue: (
            provider: string,
            owner: string,
            repo: string,
            title: string,
            body?: string,
            labels?: string[]
        ) =>
            mutation.mutateAsync({
                provider,
                owner,
                repo,
                title,
                body,
                labels,
            }),
    };
}

/**
 * Paginated search results for `is:issue involves:@me
 * sort:updated-desc`, newest first. Scoped to the active connection.
 */
export function useForgeSearchIssues() {
    const { backend } = useAppServices();
    const provider = useActiveProvider();
    const account = useForgeAccount(provider);
    const login = account.data?.login ?? null;
    const query = useInfiniteQuery(
        infiniteSearchIssuesQuery(
            { backend },
            provider,
            login,
            account.data != null
        )
    );
    const issues = (query.data?.pages ?? []).flatMap((page) => page.items);
    return { ...query, issues };
}
