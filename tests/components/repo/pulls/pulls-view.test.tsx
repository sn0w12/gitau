// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it } from "vitest";

import { PullRequestsView } from "@/components/repo/pulls/pulls-view";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    AccountProfile,
    GithubPullRequestListItem,
    RemoteInfo,
    SearchPullRequestPage,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import type { ListTab } from "@/lib/routing/repo-search";
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

const ORIGIN: RemoteInfo[] = [
    { name: "origin", url: "https://github.com/octocat/repo.git" },
];

function pull(
    overrides: Partial<GithubPullRequestListItem> = {}
): GithubPullRequestListItem {
    return {
        number: 42,
        title: "Add a thing",
        state: "open",
        stateReason: "none",
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
    remotes?: RemoteInfo[];
    page?: SearchPullRequestPage;
}

function backendWith({ signedIn = true, remotes = ORIGIN, page }: FakeOptions) {
    const calls: {
        owner: string;
        repo: string;
        state?: string;
        labels?: string[];
    }[] = [];
    const backend = {
        github: {
            account: async (): Promise<Result<AccountProfile | null>> => ({
                ok: true,
                value: signedIn ? PROFILE : null,
            }),
            listPullRequests: async (
                owner: string,
                repo: string,
                pullState?: string,
                labels?: string[]
            ): Promise<Result<SearchPullRequestPage>> => {
                calls.push({ owner, repo, state: pullState, labels });
                return {
                    ok: true,
                    value: page ?? {
                        items: [],
                        page: 1,
                        hasMore: false,
                    },
                };
            },
        },
        remotes: {
            list: async (): Promise<Result<RemoteInfo[]>> => ({
                ok: true,
                value: remotes,
            }),
        },
    };
    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        calls,
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

/** The list tab and label are controlled by the repo page's query string, so
 * a standalone render supplies them as props. */
function renderPulls(services: ReturnType<typeof backendWith>, tab: ListTab) {
    return renderWith(
        services,
        <PullRequestsView
            repoId={1}
            tab={tab}
            onTabChange={() => {}}
            label="none"
            onLabelChange={() => {}}
        />
    );
}

describe("PullRequestsView", () => {
    beforeEach(() => {
        seedSettingsForTests({});
        document.body.innerHTML = "";
    });

    it("prompts to connect while signed out", async () => {
        const services = backendWith({ signedIn: false });
        const view = renderPulls(services, "open");
        await flush();

        const shown = await waitFor(() =>
            view.container.textContent!.includes("Connect GitHub")
        );
        expect(shown).toBe(true);
        expect(services.calls).toEqual([]);
        view.unmount();
    });

    it("explains a missing github remote instead of calling the backend", async () => {
        const services = backendWith({ remotes: [] });
        const view = renderPulls(services, "open");
        await flush();

        const shown = await waitFor(() =>
            view.container.textContent!.includes("No GitHub remote")
        );
        expect(shown).toBe(true);
        expect(services.calls).toEqual([]);
        view.unmount();
    });

    it("requests open pull requests and shows the author", async () => {
        const services = backendWith({
            page: {
                items: [pull()],
                page: 1,
                hasMore: false,
            },
        });
        const view = renderPulls(services, "open");
        await flush();

        const shown = await waitFor(() =>
            view.container.textContent!.includes("Add a thing")
        );
        expect(shown).toBe(true);
        expect(services.calls).toEqual([
            {
                owner: "octocat",
                repo: "repo",
                state: "open",
                labels: [],
            },
        ]);

        const text = view.container.textContent!;
        expect(text).toContain("Open");
        expect(text).toContain("enhancement");
        expect(text).toContain("octocat");
        view.unmount();
    });

    it("separates a merged pull request from a closed one", async () => {
        const services = backendWith({
            page: {
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
        });
        const view = renderPulls(services, "open");
        await flush();

        const shown = await waitFor(() =>
            view.container.textContent!.includes("Abandoned one")
        );
        expect(shown).toBe(true);
        expect(view.container.textContent).toContain("Merged");
        expect(view.container.textContent).toContain("Closed");
        view.unmount();
    });
});
