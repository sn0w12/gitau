import { queryOptions } from "@tanstack/react-query";

import type { BackendClient } from "@/lib/backend/transport/client";
import { expectOk } from "@/lib/backend/transport/result";

import { githubKeys } from "./query-keys";

interface GithubIssuesDeps {
    backend: BackendClient;
}

export type IssueStateFilter = "open" | "closed" | "all";

/** Issues of one GitHub repository, newest activity first. */
export function repoIssuesQuery(
    deps: GithubIssuesDeps,
    owner: string | null,
    repo: string | null,
    state: IssueStateFilter,
    labels: string[],
    enabled: boolean
) {
    return queryOptions({
        queryKey: githubKeys.issues(
            owner ?? "",
            repo ?? "",
            state,
            [...labels].sort().join(",")
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.listIssues(
                    owner ?? "",
                    repo ?? "",
                    state,
                    labels
                )
            ),
        staleTime: 30_000,
        retry: false,
        enabled: enabled && owner != null && repo != null,
    });
}

/** Full issue detail for the issue page header and sidebar. */
export function issueDetailQuery(
    deps: GithubIssuesDeps,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: githubKeys.issueDetail(owner ?? "", repo ?? "", number ?? 0),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.getIssue(
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

/** Comments on the issue timeline. */
export function issueCommentsQuery(
    deps: GithubIssuesDeps,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: githubKeys.issueComments(
            owner ?? "",
            repo ?? "",
            number ?? 0
        ),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.listIssueComments(
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

/** The signed-in user's access on a repository, for gating moderation. */
export function repoPermissionsQuery(
    deps: GithubIssuesDeps,
    owner: string | null,
    repo: string | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: githubKeys.repoPermissions(owner ?? "", repo ?? ""),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.repoPermissions(
                    owner ?? "",
                    repo ?? ""
                )
            ),
        staleTime: 5 * 60_000,
        retry: false,
        enabled: enabled && owner != null && repo != null,
    });
}

/** Non-comment timeline events (labeled, assigned, closed, ...). */
export function issueEventsQuery(
    deps: GithubIssuesDeps,
    owner: string | null,
    repo: string | null,
    number: number | null,
    enabled: boolean
) {
    return queryOptions({
        queryKey: githubKeys.issueEvents(owner ?? "", repo ?? "", number ?? 0),
        queryFn: async () =>
            expectOk(
                await deps.backend.github.listIssueEvents(
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
