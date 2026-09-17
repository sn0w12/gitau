import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { BackgroundFetch } from "@/components/background-fetch";
import { AppServicesContext } from "@/contexts/services-context";
import {
    DEFAULT_FEED_INTERVAL_MS,
    MAX_SKEW_MS,
} from "@/hooks/repositories/use-background-fetch";
import type { RemoteInfo } from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import { fetchStore } from "@/stores/fetch-store";
import { repositoryStore } from "@/stores/repository-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const REPO_PATH = "C:/repos/demo";

function openRepo(repoId: number): void {
    repositoryStore.setState(() => ({
        entries: new Map([
            [REPO_PATH, { path: REPO_PATH, addedAt: Date.now(), repoId }],
        ]),
    }));
}

function noRepos(): void {
    repositoryStore.setState(() => ({ entries: new Map() }));
}

interface FetchCall {
    remote?: string;
    prune?: boolean;
}

function servicesFor(remotes: RemoteInfo[]) {
    const fetchCalls: FetchCall[] = [];
    const backend = {
        remotes: {
            list: async (): Promise<Result<RemoteInfo[]>> => ({
                ok: true,
                value: remotes,
            }),
            fetch: async (
                _repoId: number,
                options?: FetchCall
            ): Promise<Result<void>> => {
                fetchCalls.push(options ?? {});
                return { ok: true, value: undefined };
            },
        },
    } as unknown as BackendClient;
    return {
        backend,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        fetchCalls,
    };
}

function render(services: ReturnType<typeof servicesFor>) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={services}>
                <QueryClientProvider client={services.queryClient}>
                    <BackgroundFetch />
                </QueryClientProvider>
            </AppServicesContext.Provider>
        );
    });
    return {
        unmount() {
            act(() => root.unmount());
            container.remove();
        },
    };
}

/** Advance timers far enough to fire one full background-fetch cycle. */
async function advanceOneCycle() {
    await act(async () => {
        await vi.advanceTimersByTimeAsync(
            DEFAULT_FEED_INTERVAL_MS + MAX_SKEW_MS + 1_000
        );
    });
}

beforeEach(() => {
    vi.useFakeTimers();
    fetchStore.setState(() => ({
        lastFetchedByPath: {},
        inFlightByPath: {},
    }));
});

afterEach(() => {
    noRepos();
    fetchStore.setState(() => ({
        lastFetchedByPath: {},
        inFlightByPath: {},
    }));
    vi.useRealTimers();
});

describe("useBackgroundFetch", () => {
    it("fetches each open repo once on the interval and records last fetched", async () => {
        openRepo(7);
        const services = servicesFor([{ name: "origin", url: "https://x" }]);
        const view = render(services);
        try {
            await advanceOneCycle();
            expect(services.fetchCalls).toHaveLength(1);
            // Matches GitHub Desktop's `git fetch --prune`.
            expect(services.fetchCalls[0]).toMatchObject({
                remote: "origin",
                prune: true,
            });
            expect(
                fetchStore.state.lastFetchedByPath[REPO_PATH]
            ).toBeGreaterThan(0);
        } finally {
            view.unmount();
        }
    });

    it("skips repos without a remote", async () => {
        openRepo(7);
        const services = servicesFor([]);
        const view = render(services);
        try {
            await advanceOneCycle();
            expect(services.fetchCalls).toHaveLength(0);
            expect(
                fetchStore.state.lastFetchedByPath[REPO_PATH]
            ).toBeUndefined();
        } finally {
            view.unmount();
        }
    });

    it("does not fetch repos that are not open", async () => {
        noRepos();
        const services = servicesFor([{ name: "origin", url: "https://x" }]);
        const view = render(services);
        try {
            await advanceOneCycle();
            expect(services.fetchCalls).toHaveLength(0);
        } finally {
            view.unmount();
        }
    });
});
