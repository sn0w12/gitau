// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it } from "vitest";

import { AppServicesContext } from "@/contexts/services-context";
import type {
    AccountProfile,
    GithubPullRequestListItem,
    RemoteInfo,
    SearchPullRequestPage,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import { resetAppRuntime } from "@/lib/bootstrap/app-runtime";
import { PullsPage } from "@/routes/pulls-page";
import { ensureRepo, repositoryStore } from "@/stores/repository-store";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const PROFILE: AccountProfile = {
    login: "octocat",
    name: "The Octocat",
    avatarUrl: "https://avatars.githubusercontent.com/u/1",
    htmlUrl: "https://github.com/octocat",
    scopes: ["repo", "read:user"],
    connectedAtMs: 1_700_000_000_000,
};

function pull(
    overrides: Partial<GithubPullRequestListItem> = {}
): GithubPullRequestListItem {
    return {
        number: 42,
        title: "Add a thing",
        state: "open",
        labels: [{ name: "enhancement", color: "a2eeef" }],
        commentCount: 3,
        assignees: [],
        author: { login: "octocat", avatarUrl: "" },
        updatedAt: new Date(Date.now() - 60_000).toISOString(),
        mergedAt: null,
        htmlUrl: "https://github.com/octocat/repo/pull/42",
        repoFullName: "octocat/repo",
        ...overrides,
    };
}

interface FakeOptions {
    signedIn?: boolean;
    pages?: SearchPullRequestPage[];
}

function backendWith({ signedIn = true, pages = [] }: FakeOptions) {
    const calls: (number | undefined)[] = [];
    const opened: string[] = [];
    const backend = {
        github: {
            account: async (): Promise<Result<AccountProfile | null>> => ({
                ok: true,
                value: signedIn ? PROFILE : null,
            }),
            searchPullRequests: async (
                page?: number
            ): Promise<Result<SearchPullRequestPage>> => {
                calls.push(page);
                return {
                    ok: true,
                    value: pages[page === undefined ? 0 : page - 1] ?? {
                        items: [],
                        page: page ?? 1,
                        hasMore: false,
                    },
                };
            },
        },
        remotes: {
            list: async (): Promise<Result<RemoteInfo[]>> => ({
                ok: true,
                value: [],
            }),
            listByPath: async (): Promise<Result<RemoteInfo[]>> => ({
                ok: true,
                value: [
                    {
                        name: "origin",
                        url: "https://github.com/octocat/repo.git",
                    },
                ],
            }),
        },
        repositories: {
            open: async (
                path: string
            ): Promise<Result<{ id: number; path: string }>> => {
                opened.push(path);
                return { ok: true, value: { id: 7, path } };
            },
        },
    };
    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        calls,
        opened,
    };
}

function renderWith(
    services: ReturnType<typeof backendWith>,
    ui: ReactElement
) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={services}>
                <QueryClientProvider client={services.queryClient}>
                    {ui}
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

function loadMoreButton(view: { container: HTMLElement }): Element | null {
    return (
        [...view.container.querySelectorAll("button")].find(
            (candidate) => candidate.textContent === "Load more"
        ) ?? null
    );
}

function titleButton(view: { container: HTMLElement }, title: string) {
    return (
        [...view.container.querySelectorAll("button")].find(
            (candidate) => candidate.textContent === title
        ) ?? null
    );
}

async function waitFor(predicate: () => boolean): Promise<boolean> {
    const deadline = Date.now() + 2_000;
    while (Date.now() < deadline) {
        if (predicate()) return true;
        await act(async () => {
            await new Promise((resolve) => setTimeout(resolve, 25));
        });
    }
    return predicate();
}

describe("PullsPage", () => {
    beforeEach(() => {
        seedSettingsForTests({});
        repositoryStore.setState(() => ({ entries: new Map() }));
        document.body.innerHTML = "";
    });

    it("prompts to connect while signed out", async () => {
        const services = backendWith({ signedIn: false });
        const view = renderWith(services, <PullsPage />);
        await flush();

        const shown = await waitFor(() =>
            view.container.textContent!.includes("Connect GitHub")
        );
        expect(shown).toBe(true);
        expect(services.calls).toEqual([]);
        view.unmount();
    });

    it("groups pull requests by repository and names the author", async () => {
        const services = backendWith({
            pages: [
                {
                    items: [
                        pull(),
                        pull({
                            number: 7,
                            title: "Docs update",
                            repoFullName: "hubot/tools",
                            htmlUrl: "https://github.com/hubot/tools/pull/7",
                        }),
                    ],
                    page: 1,
                    hasMore: false,
                },
            ],
        });
        const view = renderWith(services, <PullsPage />);
        await flush();

        const listed = await waitFor(() =>
            view.container.textContent!.includes("Add a thing")
        );
        expect(listed).toBe(true);
        expect(services.calls).toEqual([1]);
        const text = view.container.textContent!;
        expect(text).toContain("octocat/repo");
        expect(text).toContain("hubot/tools");
        expect(text).toContain("Docs update");
        expect(text).toContain("octocat");
        expect(text).toContain("#42");
        expect(
            view.container.querySelector(".lucide-circle-dot")
        ).not.toBeNull();
        view.unmount();
    });

    it("separates a merged pull request from a closed one", async () => {
        const services = backendWith({
            pages: [
                {
                    items: [
                        pull({
                            number: 1,
                            title: "Merged one",
                            state: "closed",
                            mergedAt: new Date().toISOString(),
                        }),
                        pull({
                            number: 2,
                            title: "Abandoned one",
                            state: "closed",
                            mergedAt: null,
                        }),
                    ],
                    page: 1,
                    hasMore: false,
                },
            ],
        });
        const view = renderWith(services, <PullsPage />);
        await flush();

        const listed = await waitFor(() =>
            view.container.textContent!.includes("Abandoned one")
        );
        expect(listed).toBe(true);
        expect(
            view.container.querySelector(".lucide-git-merge")
        ).not.toBeNull();
        expect(
            view.container.querySelector(".lucide-circle-slash")
        ).not.toBeNull();
        view.unmount();
    });

    it("asks for the next page when the last one reported more", async () => {
        const services = backendWith({
            pages: [
                {
                    items: [pull()],
                    page: 1,
                    hasMore: true,
                },
                {
                    items: [pull({ number: 43, title: "Second page" })],
                    page: 2,
                    hasMore: false,
                },
            ],
        });
        const view = renderWith(services, <PullsPage />);
        await flush();

        const offered = await waitFor(() => loadMoreButton(view) !== null);
        expect(offered).toBe(true);
        await click(loadMoreButton(view)!);

        const appended = await waitFor(() =>
            view.container.textContent!.includes("Second page")
        );
        expect(appended).toBe(true);
        expect(services.calls).toEqual([1, 2]);
        view.unmount();
    });

    it("says so when no pull request involves the account", async () => {
        const services = backendWith({ pages: [] });
        const view = renderWith(services, <PullsPage />);
        await flush();

        const shown = await waitFor(() =>
            view.container.textContent!.includes("No pull requests found")
        );
        expect(shown).toBe(true);
        view.unmount();
    });

    it("opens a thread in a listed repo that was never opened", async () => {
        const services = backendWith({
            pages: [{ items: [pull()], page: 1, hasMore: false }],
        });
        // The repository store holds the durable identity the app persists,
        // so a repo nobody has clicked yet still counts as known.
        ensureRepo("C:\\repos\\repo");
        resetAppRuntime({
            backend: services.backend,
            queryClient: services.queryClient,
        });
        const view = renderWith(services, <PullsPage />);
        await flush();

        // Nothing is bound yet, so the title must not fall back to an
        // external link just because no repo is open.
        const inApp = await waitFor(
            () => titleButton(view, "Add a thing") !== null
        );
        expect(inApp).toBe(true);
        expect(view.container.querySelector('a[href*="/pull/42"]')).toBeNull();

        await click(titleButton(view, "Add a thing")!);

        const opened = await waitFor(() => services.opened.length === 1);
        expect(opened).toBe(true);
        expect(services.opened).toEqual(["C:\\repos\\repo"]);
        resetAppRuntime(null);
        view.unmount();
    });
});
