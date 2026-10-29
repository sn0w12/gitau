import { infiniteQueryOptions, queryOptions } from "@tanstack/react-query";

import type { BackendClient } from "@/lib/backend/transport/client";
import { expectOk } from "@/lib/backend/transport/result";

import { forgeKeys } from "./query-keys";

interface ForgeQueryDeps {
    backend: BackendClient;
}

/** Providers this build can talk to, with their capabilities. */
export function providersQuery(deps: ForgeQueryDeps) {
    return queryOptions({
        queryKey: forgeKeys.providers(),
        queryFn: async () => expectOk(await deps.backend.forge.providers()),
        staleTime: 5 * 60_000,
    });
}

/** Every connection that currently holds a token. */
export function connectionsQuery(deps: ForgeQueryDeps) {
    return queryOptions({
        queryKey: forgeKeys.connections(),
        queryFn: async () => expectOk(await deps.backend.forge.connections()),
        staleTime: 30_000,
    });
}

/** The connected account on one provider, `null` while signed out. */
export function accountQuery(deps: ForgeQueryDeps, provider: string) {
    return queryOptions({
        queryKey: forgeKeys.account(provider),
        queryFn: async () =>
            expectOk(await deps.backend.forge.account(provider)),
        staleTime: 60_000,
    });
}

/** Organizations the signed-in user can publish into. */
export function orgsQuery(
    deps: ForgeQueryDeps,
    provider: string,
    enabled: boolean
) {
    return queryOptions({
        queryKey: forgeKeys.orgs(provider),
        queryFn: async () =>
            expectOk(await deps.backend.forge.listOrgs(provider)),
        staleTime: 5 * 60_000,
        retry: false,
        enabled,
    });
}

/**
 * Accumulating inbox pages, newest first. Page params are 1-based to
 * match the backend; the next page comes from the `Link` header via
 * `hasMore`. Polls so the badge and list stay live without a push
 * channel. Scoped by provider and account login so one connection never
 * reads another's cached pages.
 */
export function infiniteNotificationsQuery(
    deps: ForgeQueryDeps,
    provider: string | null,
    login: string | null,
    enabled: boolean
) {
    return infiniteQueryOptions({
        queryKey: forgeKeys.notifications(provider ?? "", login ?? ""),
        queryFn: async ({ pageParam }) =>
            expectOk(
                await deps.backend.forge.listNotifications(
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

/**
 * Accumulating search results for `is:issue involves:@me
 * sort:updated-desc`, newest first. Scoped by provider and account login.
 */
export function infiniteSearchIssuesQuery(
    deps: ForgeQueryDeps,
    provider: string | null,
    login: string | null,
    enabled: boolean
) {
    return infiniteQueryOptions({
        queryKey: forgeKeys.searchIssues(provider ?? "", login ?? ""),
        queryFn: async ({ pageParam }) =>
            expectOk(
                await deps.backend.forge.searchIssues(provider ?? "", pageParam)
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
