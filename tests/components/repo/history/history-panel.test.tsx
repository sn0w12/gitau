import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { describe, expect, it } from "vitest";

import { HistoryPanel } from "@/components/repo/history/history-panel";
import { ConfirmProvider } from "@/contexts/confirm-context";
import { AppServicesContext } from "@/contexts/services-context";
import { TabContext } from "@/contexts/tab-context";
import type { CommitSummary, HistoryPage } from "@/lib/backend/protocol";
import { historyKeys } from "@/lib/backend/queries/query-keys";
import { historyPageQuery } from "@/lib/backend/queries/repository-queries";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import type { AppServices } from "@/lib/bootstrap/app-runtime";
import { seedSettingsForTests } from "@/stores/settings-store";

function commit(overrides: Partial<CommitSummary> = {}): CommitSummary {
    return {
        id: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b",
        treeId: "tree",
        parentIds: [],
        summaryLine: "Add history prefetching",
        message: "Add history prefetching",
        author: {
            name: "Ada",
            email: "ada@example.com",
            timeSeconds: 1_700_000_000,
            timeOffsetMinutes: 0,
        },
        committer: {
            name: "Ada",
            email: "ada@example.com",
            timeSeconds: 1_700_000_000,
            timeOffsetMinutes: 0,
        },
        filesChanged: 2,
        additions: 10,
        deletions: 4,
        tags: [],
        ...overrides,
    };
}

const page = (commits: CommitSummary[], hasMore = false): HistoryPage => ({
    snapshotId: 1,
    generation: 1,
    commits,
    hasMore,
});

function fakeBackend(pageResult: Result<HistoryPage>): AppServices {
    const backend = {
        history: {
            page: async (): Promise<Result<HistoryPage>> => pageResult,
        },
    };
    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
    };
}

/**
 * Serves one configured page per call (regardless of the requested skip),
 * then keeps returning the last entry. Tracks how many times the backend
 * was hit so tests can assert pagination stopped.
 */
function pagedBackend(pages: Result<HistoryPage>[]): AppServices & {
    callCount: () => number;
} {
    let calls = 0;
    const backend = {
        history: {
            page: async (): Promise<Result<HistoryPage>> => {
                const result = pages[Math.min(calls, pages.length - 1)];
                calls += 1;
                return result;
            },
        },
    };
    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        callCount: () => calls,
    };
}

function renderWith(services: AppServices, ui: ReactElement) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={services}>
                <QueryClientProvider client={services.queryClient}>
                    <ConfirmProvider>
                        <TabContext.Provider value="tab-test">
                            {ui}
                        </TabContext.Provider>
                    </ConfirmProvider>
                </QueryClientProvider>
            </AppServicesContext.Provider>
        );
    });
    return {
        container,
        unmount() {
            act(() => root.unmount());
            container.remove();
        },
    };
}

async function flushMicrotasks(): Promise<void> {
    await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 20));
    });
}

describe("HistoryPanel", () => {
    it("renders cached commits immediately without skeletons", () => {
        seedSettingsForTests({});
        const services = fakeBackend({ ok: true, value: page([commit()]) });
        // Prime the infinite cache exactly like a prior visit would have.
        act(() => {
            services.queryClient.setQueryData(historyKeys.infinite(9), {
                pages: [
                    page([
                        commit(),
                        commit({
                            id: "2b2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b",
                            summaryLine: "Earlier commit",
                        }),
                    ]),
                ],
                pageParams: [0],
            });
        });

        const view = renderWith(
            services,
            <HistoryPanel repoId={9} selectedId={null} onSelect={() => {}} />
        );

        const list = view.container.querySelector('[data-slot="history-list"]');
        expect(list).not.toBeNull();
        expect(list?.textContent).toContain("Add history prefetching");
        expect(list?.textContent).toContain("Earlier commit");
        // No loading skeletons on the primed path.
        expect(
            view.container.querySelector('[data-slot="skeleton"]')
        ).toBeNull();
        view.unmount();
    });

    it("shows skeletons only when no data ever arrived", async () => {
        seedSettingsForTests({});
        const view = renderWith(
            fakeBackend({ ok: true, value: page([]) }),
            <HistoryPanel repoId={11} selectedId={null} onSelect={() => {}} />
        );
        expect(
            view.container.querySelector('[data-slot="history-list"]')
        ).toBeNull();

        await flushMicrotasks();
        // A resolved-but-empty repository renders the explicit empty state.
        expect(
            view.container.querySelector('[data-slot="history-empty"]')
        ).not.toBeNull();
        view.unmount();
    });

    it("loads further pages when scrolled near the bottom", async () => {
        seedSettingsForTests({});
        const services = pagedBackend([
            {
                ok: true,
                value: page(
                    [commit({ summaryLine: "First page commit" })],
                    true
                ),
            },
            {
                ok: true,
                value: page(
                    [
                        commit({
                            id: "3c3c3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b",
                            summaryLine: "Second page commit",
                        }),
                    ],
                    false
                ),
            },
        ]);

        const view = renderWith(
            services,
            <HistoryPanel repoId={21} selectedId={null} onSelect={() => {}} />
        );

        // First page arrives.
        await flushMicrotasks();
        expect(view.container.textContent).toContain("First page commit");
        expect(services.callCount()).toBe(1);

        // Give the viewport real geometry, then scroll near the bottom.
        const viewport = view.container.querySelector<HTMLElement>(
            '[data-slot="scroll-area-viewport"]'
        );
        expect(viewport).not.toBeNull();
        Object.defineProperty(viewport!, "clientHeight", {
            value: 600,
            configurable: true,
        });
        Object.defineProperty(viewport!, "scrollHeight", {
            value: 900,
            configurable: true,
        });
        await act(async () => {
            viewport!.dispatchEvent(new Event("scroll", { bubbles: true }));
            await new Promise((resolve) => setTimeout(resolve, 20));
        });

        await flushMicrotasks();
        const list = view.container.querySelector('[data-slot="history-list"]');
        expect(list?.textContent).toContain("Second page commit");
        // Exhaustion stops further fetching.
        expect(services.callCount()).toBe(2);
        view.unmount();
    });

    it("does not auto-load while inactive", async () => {
        seedSettingsForTests({});
        const services = pagedBackend([
            {
                ok: true,
                value: page(
                    [commit({ summaryLine: "Only visible page" })],
                    true
                ),
            },
            { ok: true, value: page([], false) },
        ]);

        const view = renderWith(
            services,
            <HistoryPanel
                repoId={31}
                selectedId={null}
                onSelect={() => {}}
                active={false}
            />
        );
        await flushMicrotasks();

        // The warm first page loads, but the near-bottom auto-fill must
        // stay dormant for kept-mounted hidden tabs (they measure 0x0).
        expect(services.callCount()).toBe(1);
        expect(view.container.textContent).toContain("Only visible page");
        view.unmount();
    });

    it("prefetch query options resolve through the typed client", async () => {
        seedSettingsForTests({});
        const services = fakeBackend({ ok: true, value: page([commit()]) });
        const data = await services.queryClient.fetchQuery(
            historyPageQuery({ backend: services.backend }, 13)
        );
        expect(data.commits).toHaveLength(1);
    });

    it("sends the debounced search term and shows the filtered page", async () => {
        seedSettingsForTests({});
        const searches: string[] = [];
        const services: AppServices = {
            backend: {
                history: {
                    page: async (
                        _repoId: number,
                        query: { search?: string }
                    ): Promise<Result<HistoryPage>> => {
                        const search = query.search ?? "";
                        searches.push(search);
                        if (search.toLowerCase().includes("security")) {
                            return {
                                ok: true,
                                value: page([
                                    commit({
                                        id: "4d4d4d4d4e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b",
                                        summaryLine: "Fix security hole",
                                    }),
                                ]),
                            };
                        }
                        return {
                            ok: true,
                            value: page([
                                commit({ summaryLine: "Other commit" }),
                            ]),
                        };
                    },
                },
            } as unknown as BackendClient,
            queryClient: new QueryClient({
                defaultOptions: { queries: { retry: false } },
            }),
        };

        const view = renderWith(
            services,
            <HistoryPanel repoId={41} selectedId={null} onSelect={() => {}} />
        );
        await flushMicrotasks();
        expect(view.container.textContent).toContain("Other commit");

        const input = view.container.querySelector<HTMLInputElement>(
            '[aria-label="Filter history"]'
        );
        expect(input).not.toBeNull();
        const setter = Object.getOwnPropertyDescriptor(
            window.HTMLInputElement.prototype,
            "value"
        )!.set!;
        act(() => {
            setter.call(input, "  Security  ");
            input!.dispatchEvent(new Event("input", { bubbles: true }));
        });
        // The trimmed term is debounced before it hits the backend.
        await act(async () => {
            await new Promise((resolve) => setTimeout(resolve, 350));
        });
        await flushMicrotasks();

        // The raw input is trimmed on the frontend; case folding lives in
        // the Rust engine.
        expect(searches).toContain("Security");
        expect(view.container.textContent).toContain("Fix security hole");
        expect(
            view.container.querySelector('[data-slot="history-list"]')
        ).not.toBeNull();
        view.unmount();
    });

    it("reports a search with no matches distinctly", async () => {
        seedSettingsForTests({});
        const services: AppServices = {
            backend: {
                history: {
                    page: async (
                        _repoId: number,
                        query: { search?: string }
                    ): Promise<Result<HistoryPage>> => {
                        const search = query.search ?? "";
                        if (search) {
                            return { ok: true, value: page([]) };
                        }
                        return { ok: true, value: page([commit()]) };
                    },
                },
            } as unknown as BackendClient,
            queryClient: new QueryClient({
                defaultOptions: { queries: { retry: false } },
            }),
        };

        const view = renderWith(
            services,
            <HistoryPanel repoId={51} selectedId={null} onSelect={() => {}} />
        );
        await flushMicrotasks();

        const input = view.container.querySelector<HTMLInputElement>(
            '[aria-label="Filter history"]'
        );
        const setter = Object.getOwnPropertyDescriptor(
            window.HTMLInputElement.prototype,
            "value"
        )!.set!;
        act(() => {
            setter.call(input, "nope");
            input!.dispatchEvent(new Event("input", { bubbles: true }));
        });
        await act(async () => {
            await new Promise((resolve) => setTimeout(resolve, 350));
        });
        await flushMicrotasks();

        const empty = view.container.querySelector(
            '[data-slot="history-empty"]'
        );
        expect(empty).not.toBeNull();
        expect(empty?.textContent).toContain("No commits match");
        view.unmount();
    });
});
