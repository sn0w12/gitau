import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { describe, expect, it } from "vitest";

import { RepoFetchButton } from "@/components/repo/sync/fetch-button";
import { ToastProvider } from "@/components/ui/toast";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    AccountProfile,
    BranchInfo,
    RepoListing,
    RepoSnapshot,
    WorkflowOutcome,
    PushOutcome,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import { fetchStore, hydrateFetchTimestamps } from "@/stores/fetch-store";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

function branch(overrides: Partial<BranchInfo> & { name: string }): BranchInfo {
    return {
        target: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b",
        isHead: false,
        ...overrides,
    };
}

function backendWith(overrides: {
    listing?: BranchInfo[];
    remotes?: { name: string; url?: string }[];
    headBranchName?: string;
}) {
    const snapshot: Result<RepoSnapshot> = {
        ok: true,
        value: {
            id: 5,
            generation: 1,
            gitDir: "/tmp/.git",
            head: overrides.headBranchName
                ? {
                      state: "attached" as const,
                      branch: overrides.headBranchName,
                      target: "abc",
                  }
                : { state: "detached", target: "abc" },
            shaKind: "sha1",
        },
    };
    const listing: Result<RepoListing> = {
        ok: true,
        value: { branches: (overrides.listing ?? []).map(branch), tags: [] },
    };
    const remotesResult = overrides.remotes ?? [{ name: "origin" }];

    const fetchCalls: number[] = [];
    const pushOptions: Record<string, unknown>[] = [];
    let pullOutcome: Result<WorkflowOutcome> = {
        ok: true,
        value: { outcome: "fastForwarded", head: "abc" },
    };
    let pushOutcomes: Result<PushOutcome[]> = {
        ok: true,
        value: [{ reference: "refs/heads/main", accepted: true }],
    };

    const backend = {
        repositories: {
            snapshot: async (): Promise<Result<RepoSnapshot>> => snapshot,
        },
        refs: {
            listBranchesAndTags: async (): Promise<Result<RepoListing>> =>
                listing,
        },
        remotes: {
            list: async (): Promise<
                Result<{ name: string; url?: string }[]>
            > => ({
                ok: true,
                value: remotesResult,
            }),
            fetch: async (): Promise<Result<void>> => {
                fetchCalls.push(Date.now());
                return { ok: true, value: undefined };
            },
            pull: async (
                _repoId: number,
                _options: unknown
            ): Promise<Result<WorkflowOutcome>> => {
                return pullOutcome;
            },
            push: async (
                _repoId: number,
                options: Record<string, unknown>
            ): Promise<Result<PushOutcome[]>> => {
                pushOptions.push(options);
                return pushOutcomes;
            },
        },
        github: {
            account: async (): Promise<Result<AccountProfile | null>> => ({
                ok: true,
                value: null,
            }),
            listOrgs: async (): Promise<Result<{ login: string }[]>> => ({
                ok: true,
                value: [],
            }),
        },
    };
    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        fetchCalls: () => fetchCalls.length,
        pushOptions,
        setPullOutcome(next: Result<WorkflowOutcome>) {
            pullOutcome = next;
        },
        setPushOutcomes(next: Result<PushOutcome[]>) {
            pushOutcomes = next;
        },
    };
}

type Services = ReturnType<typeof backendWith>;

function renderWith(services: Services, ui: ReactElement) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={services}>
                <QueryClientProvider client={services.queryClient}>
                    <ToastProvider>{ui}</ToastProvider>
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

async function flush(): Promise<void> {
    await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 20));
    });
}

async function click(element: Element): Promise<void> {
    await act(async () => {
        element.dispatchEvent(
            new MouseEvent("click", { bubbles: true, cancelable: true })
        );
        await new Promise((resolve) => setTimeout(resolve, 10));
    });
}

async function waitFor(predicate: () => boolean): Promise<boolean> {
    const deadline = Date.now() + 1_000;
    while (Date.now() < deadline) {
        if (predicate()) return true;
        await new Promise((resolve) => setTimeout(resolve, 25));
    }
    return predicate();
}

const REPO_PATH = "C:/repos/demo";

describe("RepoFetchButton", () => {
    it("renders Fetch Origin when in sync and records the fetched time", async () => {
        seedSettingsForTests({});
        hydrateFetchTimestamps({});
        const services = backendWith({
            headBranchName: "main",
            listing: [
                branch({
                    name: "main",
                    isHead: true,
                    upstream: {
                        remote: "origin",
                        branch: "main",
                        ahead: 0,
                        behind: 0,
                    },
                }),
            ],
        });

        const view = renderWith(
            services,
            <RepoFetchButton repoId={5} repoPath={REPO_PATH} />
        );
        await flush();

        expect(view.container.textContent).toContain("Fetch Origin");
        await click(
            view.container.querySelector('[data-testid="fetch-button"]')!
        );

        const settled = await waitFor(() => services.fetchCalls() === 1);
        expect(settled).toBe(true);
        expect(fetchStore.state.lastFetchedByPath[REPO_PATH]).toBeGreaterThan(
            0
        );
        view.unmount();
    });

    it("publishes a branch without upstream via push --set-upstream", async () => {
        seedSettingsForTests({});
        hydrateFetchTimestamps({});
        const services = backendWith({
            headBranchName: "main",
            listing: [branch({ name: "main", isHead: true })],
        });

        const view = renderWith(
            services,
            <RepoFetchButton repoId={5} repoPath={REPO_PATH} />
        );
        await flush();

        expect(view.container.textContent).toContain("Publish branch");
        await click(
            view.container.querySelector('[data-testid="fetch-button"]')!
        );

        const settled = await waitFor(() => services.pushOptions.length > 0);
        expect(settled).toBe(true);
        expect(services.pushOptions[0]).toMatchObject({ setUpstream: true });
        view.unmount();
    });

    it("publishes to GitHub directly without any configured remote", async () => {
        seedSettingsForTests({});
        hydrateFetchTimestamps({});
        const services = backendWith({
            headBranchName: "main",
            remotes: [],
            listing: [branch({ name: "main", isHead: true })],
        });

        const view = renderWith(
            services,
            <RepoFetchButton repoId={5} repoPath={REPO_PATH} />
        );
        await flush();

        const trigger = view.container.querySelector(
            '[data-testid="publish-repository-button"]'
        );
        expect(trigger).not.toBeNull();
        expect(view.container.textContent).toContain("Publish repository");

        await click(trigger!);
        const opened = await waitFor(
            () =>
                document.body.querySelector(
                    '[data-testid="publish-dialog"]'
                ) !== null
        );
        expect(opened).toBe(true);
        view.unmount();
    });
});
