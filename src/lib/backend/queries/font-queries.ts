import { queryOptions } from "@tanstack/react-query";

import type { BackendClient } from "@/lib/backend/transport/client";
import { expectOk } from "@/lib/backend/transport/result";

import { fontKeys } from "./query-keys";

interface FontQueryDeps {
    backend: BackendClient;
}

/** Installed font families; the backend caches them for the process. */
export function systemFontsQuery(deps: FontQueryDeps) {
    return queryOptions({
        queryKey: fontKeys.system,
        queryFn: async () => expectOk(await deps.backend.fonts.list()),
        staleTime: Number.POSITIVE_INFINITY,
        retry: false,
    });
}
