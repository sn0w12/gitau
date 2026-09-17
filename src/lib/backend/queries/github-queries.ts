import { queryOptions } from "@tanstack/react-query";

import type { BackendClient } from "@/lib/backend/transport/client";
import { expectOk } from "@/lib/backend/transport/result";

import { githubKeys } from "./query-keys";

interface GithubQueryDeps {
    backend: BackendClient;
}

/** The connected GitHub account, `null` while signed out. */
export function accountQuery(deps: GithubQueryDeps) {
    return queryOptions({
        queryKey: githubKeys.account(),
        queryFn: async () => expectOk(await deps.backend.github.account()),
        staleTime: 60_000,
    });
}

/** Organizations the signed-in user can publish into. */
export function orgsQuery(deps: GithubQueryDeps, enabled: boolean) {
    return queryOptions({
        queryKey: githubKeys.orgs(),
        queryFn: async () => expectOk(await deps.backend.github.listOrgs()),
        staleTime: 5 * 60_000,
        retry: false,
        enabled,
    });
}
