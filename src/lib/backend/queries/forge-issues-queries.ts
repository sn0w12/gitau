import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";

import type { BackendClient } from "@/lib/backend/transport/client";
import { expectOk } from "@/lib/backend/transport/result";

import { forgeKeys } from "./query-keys";

interface ForgeIssuesDeps {
    backend: BackendClient;
}

export type IssueStateFilter = "open" | "closed" | "all";

/** Issues of one repository, newest activity first, paginated. */
export function infiniteRepoIssuesQuery(
    deps: ForgeIssuesDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    state: IssueStateFilter,
    labels: string[],
    enabled: boolean
) {
    return infiniteQueryOptions({
        queryKey: forgeKeys.issues(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            state,
            [...labels].sort().join(",")
        ),
        queryFn: async ({ pageParam }) =>
            expectOk(
                await deps.backend.forge.listIssues(
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

/** Full issue detail for the issue page header and sidebar. */
export function issueDetailQuery(
    deps: ForgeIssuesDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.issueDetail(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            number ?? 0
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.getIssue(
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

/** Comments on the issue timeline. */
export function issueCommentsQuery(
    deps: ForgeIssuesDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.issueComments(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            number ?? 0
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.listIssueComments(
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

/** The signed-in user's access on a repository, for gating moderation. */
export function repoPermissionsQuery(
    deps: ForgeIssuesDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.repoPermissions(
            provider ?? "",
            owner ?? "",
            repo ?? ""
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.repoPermissions(
                    provider ?? "",
                    owner ?? "",
                    repo ?? ""
                )
            ),
        staleTime: 5 * 60_000,
        retry: false,
        enabled: enabled && provider != null && owner != null && repo != null,
    });
}

export function repoLabelsQuery(
    deps: ForgeIssuesDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.labels(provider ?? "", owner ?? "", repo ?? ""),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.listLabels(
                    provider ?? "",
                    owner ?? "",
                    repo ?? ""
                )
            ),
        staleTime: 5 * 60_000,
        retry: false,
        enabled: enabled && provider != null && owner != null && repo != null,
    });
}

/** Non-comment timeline events (labeled, assigned, closed, ...). */
export function issueEventsQuery(
    deps: ForgeIssuesDeps,
    provider: string | null,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.issueEvents(
            provider ?? "",
            owner ?? "",
            repo ?? "",
            number ?? 0
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.forge.listIssueEvents(
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
