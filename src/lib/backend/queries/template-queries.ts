import { queryOptions } from "@tanstack/react-query";

import type { BackendClient } from "@/lib/backend/transport/client";
import { expectOk } from "@/lib/backend/transport/result";

import { templateKeys } from "./query-keys";

interface TemplateQueryDeps {
    backend: BackendClient;
}

/** Embedded .gitignore presets; static data, cached for the session. */
export function gitignoreTemplatesQuery(deps: TemplateQueryDeps) {
    return queryOptions({
        queryKey: templateKeys.gitignore,
        queryFn: async () =>
            expectOk(await deps.backend.repositories.gitignoreTemplates()),
        staleTime: Infinity,
    });
}

/** Embedded license presets; static data, cached for the session. */
export function licensesQuery(deps: TemplateQueryDeps) {
    return queryOptions({
        queryKey: templateKeys.licenses,
        queryFn: async () =>
            expectOk(await deps.backend.repositories.licenses()),
        staleTime: Infinity,
    });
}
