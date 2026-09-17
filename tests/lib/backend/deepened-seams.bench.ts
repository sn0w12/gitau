import { bench, describe } from "vitest";

import type { HistoryPage } from "@/lib/backend/protocol";
import { historyKeyFor, historyKeys } from "@/lib/backend/queries/query-keys";
import { fetchHistoryPage } from "@/lib/backend/queries/repository-queries";
import { diffSessionRegistry } from "@/lib/backend/streams/diff-session-registry";
import { graphSessionRegistry } from "@/lib/backend/streams/graph-session-registry";
import {
    acquireDiff,
    acquireGraph,
    releaseStream,
} from "@/lib/backend/streams/unified-streams";
import type { BackendClient } from "@/lib/backend/transport/client";
import { expectOk } from "@/lib/backend/transport/result";
import type { Result } from "@/lib/backend/transport/result";

const page: HistoryPage = {
    snapshotId: 1,
    generation: 2,
    commits: [],
    hasMore: false,
};

const historyStub = {
    history: {
        page: async (): Promise<Result<HistoryPage>> => ({
            ok: true,
            value: page,
        }),
    },
} as unknown as BackendClient;

const streamStub = {} as BackendClient;

describe("history keys", () => {
    const query = { search: "fix" };
    bench("direct key builder (before)", () => {
        historyKeys.page(1, query);
    });
    bench("single owner historyKeyFor (after)", () => {
        historyKeyFor(1, "page", query);
    });
});

describe("history fetch", () => {
    const deps = { backend: historyStub };
    bench("inline page fetch (before)", async () => {
        const fetched = await deps.backend.history.page(7, {
            limit: 100,
            skip: 0,
            search: "fix",
        });
        expectOk(fetched);
    });
    bench("shared fetchHistoryPage (after)", async () => {
        await fetchHistoryPage(deps, 7, {
            limit: 100,
            skip: 0,
            search: "fix",
        });
    });
});

describe("stream registries", () => {
    bench("diff acquire plus release (before)", () => {
        const controller = diffSessionRegistry.acquire(streamStub, {
            repoId: 1,
        });
        diffSessionRegistry.release(controller);
    });
    bench("unified acquireDiff plus releaseStream (after)", () => {
        const controller = acquireDiff(streamStub, { repoId: 1 });
        releaseStream(controller);
    });
    bench("graph acquire plus release (before)", () => {
        const controller = graphSessionRegistry.acquire(streamStub, {
            repoId: 1,
        });
        graphSessionRegistry.release(controller);
    });
    bench("unified acquireGraph plus releaseStream (after)", () => {
        const controller = acquireGraph(streamStub, { repoId: 1 });
        releaseStream(controller);
    });
});
