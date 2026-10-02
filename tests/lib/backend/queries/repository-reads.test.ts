import { QueryClient } from "@tanstack/query-core";
import { describe, expect, it } from "vitest";

import { invalidateRepository } from "@/lib/backend/mutations/invalidation";
import type { HistoryPage } from "@/lib/backend/protocol";
import type { HistoryPageQueryLike } from "@/lib/backend/queries/query-keys";
import {
    historyKeyFor,
    historyKeys,
    repositoryKeys,
} from "@/lib/backend/queries/query-keys";
import {
    fetchHistoryPage,
    historyPageQuery,
    historyPageSize,
    infiniteHistoryPageQuery,
    rareReadQuery,
} from "@/lib/backend/queries/repository-queries";
import type { BackendClient } from "@/lib/backend/transport/client";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import type { Result } from "@/lib/backend/transport/result";
import { seedSettingsForTests } from "@/stores/settings-store";

const page: HistoryPage = {
    snapshotId: 1,
    generation: 2,
    commits: [],
    hasMore: false,
    unpushed: [],
};

function depsWith(
    pageFn: (repoId: number, query: object) => Promise<Result<HistoryPage>>
) {
    const backend = {
        history: { page: pageFn },
    } as unknown as BackendClient;
    return { backend };
}

describe("historyKeyFor", () => {
    it("keeps page and infinite shapes on distinct keys", () => {
        const query: HistoryPageQueryLike = { search: "fix" };
        const pageKey = historyKeyFor(1, "page", query);
        const infiniteKey = historyKeyFor(1, "infinite", query);
        expect(pageKey).toEqual(historyKeys.page(1, query));
        expect(infiniteKey).toEqual(historyKeys.infinite(1, "fix"));
        expect(pageKey).not.toEqual(infiniteKey);
    });

    it("defaults an empty infinite query to the empty search slot", () => {
        expect(historyKeyFor(1, "infinite", {})).toEqual(
            historyKeys.infinite(1, "")
        );
    });
});

describe("fetchHistoryPage", () => {
    it("merges limit, skip, and filters into one backend call", async () => {
        const seen: Array<[number, object]> = [];
        const deps = depsWith(async (repoId, query) => {
            seen.push([repoId, query]);
            return { ok: true, value: page };
        });
        const result = await fetchHistoryPage(deps, 7, {
            limit: 100,
            skip: 200,
            search: "fix",
        });
        expect(result).toBe(page);
        expect(seen).toEqual([[7, { limit: 100, skip: 200, search: "fix" }]]);
    });

    it("surfaces backend errors as rejections for the query layer", async () => {
        const deps = depsWith(async () => ({
            ok: false,
            error: new GitBackendError({
                code: "repoNotFound",
                message: "gone",
                retryable: false,
            }),
        }));
        await expect(
            fetchHistoryPage(deps, 7, { limit: 100, skip: 0 })
        ).rejects.toMatchObject({ code: "repoNotFound" });
    });
});

describe("history page size", () => {
    function sized(): ReturnType<typeof depsWith> {
        return depsWith(async () => ({ ok: true, value: page }));
    }

    it("reads the configured page size", () => {
        seedSettingsForTests({ historyPageSize: 25 });
        expect(historyPageSize()).toBe(25);
    });

    it("defaults the single page limit to the setting and keys it", () => {
        seedSettingsForTests({ historyPageSize: 25 });
        expect(historyPageQuery(sized(), 4).queryKey).toEqual(
            historyKeys.page(4, { limit: 25 })
        );
    });

    it("lets an explicit caller limit override the setting", () => {
        seedSettingsForTests({ historyPageSize: 25 });
        expect(historyPageQuery(sized(), 4, { limit: 5 }).queryKey).toEqual(
            historyKeys.page(4, { limit: 5 })
        );
    });

    /**
     * Skip offsets are multiples of the page size, so a cached walk built at
     * one size cannot be continued at another without dropping commits.
     */
    it("keys the infinite query by page size", () => {
        seedSettingsForTests({ historyPageSize: 25 });
        expect(infiniteHistoryPageQuery(sized(), 4).queryKey).toEqual(
            historyKeys.infinite(4, "", 25)
        );
    });

    it("derives the next skip from the configured page size", () => {
        seedSettingsForTests({ historyPageSize: 25 });
        const options = infiniteHistoryPageQuery(sized(), 4);
        const loaded = [page, page];
        expect(
            options.getNextPageParam?.(
                { ...page, hasMore: true },
                loaded,
                25,
                [0, 25]
            )
        ).toBe(50);
        expect(
            options.getNextPageParam?.(
                { ...page, hasMore: false },
                loaded,
                25,
                [0, 25]
            )
        ).toBeUndefined();
    });
});

describe("rareReadQuery", () => {
    it("reuses each read's key without caller key construction", () => {
        const deps = depsWith(async () => ({ ok: true, value: page }));
        expect(rareReadQuery(deps, 3, { kind: "listing" }).queryKey).toEqual(
            repositoryKeys.listing(3)
        );
        expect(
            rareReadQuery(deps, 3, {
                kind: "commitDetail",
                revision: "abc",
            }).queryKey
        ).toEqual(historyKeys.commitDetail(3, "abc", true));
    });
});

describe("invalidateRepository scope fan-out", () => {
    async function seed(): Promise<QueryClient> {
        const client = new QueryClient();
        client.setQueryData(repositoryKeys.snapshot(1), { generation: 2 });
        client.setQueryData(repositoryKeys.status(1, undefined), {});
        client.setQueryData(historyKeys.page(1, {}), page);
        client.setQueryData(historyKeys.infinite(1, ""), {
            pages: [page],
            pageParams: [0],
        });
        client.setQueryData(historyKeys.commitDetail(1, "abc", true), {});
        return client;
    }

    function invalidated(client: QueryClient, key: readonly unknown[]) {
        return client.getQueryState(key)?.isInvalidated;
    }

    it("history scope sweeps page, infinite, and commit", async () => {
        const client = await seed();
        await invalidateRepository(client, 1, ["history"]);
        expect(invalidated(client, historyKeys.page(1, {}))).toBe(true);
        expect(invalidated(client, historyKeys.infinite(1, ""))).toBe(true);
        expect(
            invalidated(client, historyKeys.commitDetail(1, "abc", true))
        ).toBe(true);
        expect(
            invalidated(client, repositoryKeys.status(1, undefined))
        ).not.toBe(true);
    });

    it("always refreshes the generation-bearing snapshot", async () => {
        const client = await seed();
        await invalidateRepository(client, 1, ["status"]);
        expect(invalidated(client, repositoryKeys.snapshot(1))).toBe(true);
        expect(invalidated(client, repositoryKeys.status(1, undefined))).toBe(
            true
        );
        expect(invalidated(client, historyKeys.page(1, {}))).not.toBe(true);
    });
});
