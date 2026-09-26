import { queryOptions } from "@tanstack/react-query";

import type { BackendClient } from "@/lib/backend/transport/client";
import { expectOk } from "@/lib/backend/transport/result";

import { highlightKeys } from "./query-keys";

interface HighlightQueryDeps {
    backend: BackendClient;
}

/** Compact identity for cache keys; snippets are immutable per text. */
export function hashSnippetText(text: string): string {
    let hash = 0x811c9dc5;
    for (let index = 0; index < text.length; index++) {
        hash ^= text.charCodeAt(index);
        hash = Math.imul(hash, 0x01000193);
    }
    return `${text.length}:${(hash >>> 0).toString(16)}`;
}

/**
 * Syntax highlighting for one code fence. Snippets never change for the
 * same text, so they never go stale; the theme pair is part of the key
 * so a theme change re-highlights.
 */
export function snippetQuery(
    deps: HighlightQueryDeps,
    language: string,
    text: string,
    lightTheme: string,
    darkTheme: string,
    enabled: boolean
) {
    return queryOptions({
        queryKey: highlightKeys.snippet(
            language,
            hashSnippetText(text),
            lightTheme,
            darkTheme
        ),
        queryFn: async () =>
            expectOk(await deps.backend.highlight.code(language, text)),
        staleTime: Number.POSITIVE_INFINITY,
        gcTime: 30 * 60_000,
        retry: false,
        enabled,
    });
}
