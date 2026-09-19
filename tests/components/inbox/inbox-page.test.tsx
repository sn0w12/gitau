// @vitest-environment jsdom
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it } from "vitest";

import { ToastProvider } from "@/components/ui/toast";
import { ConfirmProvider } from "@/contexts/confirm-context";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    AccountProfile,
    GithubNotification,
    NotificationPage,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import { InboxPage } from "@/routes/inbox-page";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

function thread(
    overrides: Partial<GithubNotification> = {}
): GithubNotification {
    return {
        id: "1",
        unread: true,
        reason: "mention",
        subjectTitle: "Fix the bug",
        subjectType: "PullRequest",
        repoFullName: "octocat/repo",
        htmlUrl: "https://github.com/octocat/repo/pull/42",
        subjectUrl: "https://api.github.com/repos/octocat/repo/pulls/42",
        updatedAt: new Date(Date.now() - 60_000).toISOString(),
        ...overrides,
    };
}

const PROFILE: AccountProfile = {
    login: "octocat",
    name: "The Octocat",
    avatarUrl: "https://avatars.githubusercontent.com/u/1",
    htmlUrl: "https://github.com/octocat",
    scopes: ["repo", "read:user", "workflow", "notifications"],
    connectedAtMs: 1_700_000_000_000,
};

function backendWith(pages: NotificationPage[]) {
    let markReadCalls: string[] = [];
    let markAllCalls = 0;
    let resolveCalls: string[] = [];

    const backend = {
        github: {
            account: async (): Promise<Result<AccountProfile | null>> => ({
                ok: true,
                value: PROFILE,
            }),
            listNotifications: async (
                page?: number
            ): Promise<Result<NotificationPage>> => {
                const next = pages[page === undefined ? 0 : page - 1] ?? {
                    notifications: [],
                    page: page ?? 1,
                    hasMore: false,
                };
                return { ok: true, value: next };
            },
            markNotificationRead: async (
                threadId: string
            ): Promise<Result<void>> => {
                markReadCalls.push(threadId);
                for (const page of pages) {
                    for (const item of page.notifications) {
                        if (item.id === threadId) item.unread = false;
                    }
                }
                return { ok: true, value: undefined };
            },
            markAllNotificationsRead: async (): Promise<Result<void>> => {
                markAllCalls += 1;
                for (const page of pages) {
                    for (const item of page.notifications) item.unread = false;
                }
                return { ok: true, value: undefined };
            },
            resolveSubjectUrl: async (
                subjectUrl: string
            ): Promise<Result<string | null>> => {
                resolveCalls.push(subjectUrl);
                return {
                    ok: true,
                    value: "https://github.com/octocat/repo/releases/tag/v2.0",
                };
            },
        },
        icons: {
            resolve: async (): Promise<
                Result<{ dataUrl: string; contentType: string }>
            > => ({
                ok: true,
                value: {
                    dataUrl: "data:image/png;base64,AAA",
                    contentType: "image/png",
                },
            }),
        },
    };

    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        markReadCalls: () => markReadCalls,
        markAllCalls: () => markAllCalls,
        resolveCalls: () => resolveCalls,
    };
}

function backendSignedOut() {
    const backend = {
        github: {
            account: async (): Promise<Result<AccountProfile | null>> => ({
                ok: true,
                value: null,
            }),
            listNotifications: async (): Promise<Result<NotificationPage>> => ({
                ok: true,
                value: { notifications: [], page: 1, hasMore: false },
            }),
            markNotificationRead: async (): Promise<Result<void>> => ({
                ok: true,
                value: undefined,
            }),
            markAllNotificationsRead: async (): Promise<Result<void>> => ({
                ok: true,
                value: undefined,
            }),
        },
        icons: {
            resolve: async (): Promise<
                Result<{ dataUrl: string; contentType: string }>
            > => ({
                ok: true,
                value: {
                    dataUrl: "data:image/png;base64,AAA",
                    contentType: "image/png",
                },
            }),
        },
    };

    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
    };
}

type Services =
    | ReturnType<typeof backendWith>
    | ReturnType<typeof backendSignedOut>;

function renderWith(services: Services, ui: ReactElement) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={services}>
                <QueryClientProvider client={services.queryClient}>
                    <ConfirmProvider>
                        <ToastProvider>{ui}</ToastProvider>
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
    const deadline = Date.now() + 2_000;
    while (Date.now() < deadline) {
        if (predicate()) return true;
        await new Promise((resolve) => setTimeout(resolve, 25));
    }
    return predicate();
}

describe("InboxPage", () => {
    beforeEach(() => {
        seedSettingsForTests({});
        document.body.innerHTML = "";
    });

    it("prompts to connect while signed out", async () => {
        const services = backendSignedOut();
        const view = renderWith(services, <InboxPage />);
        await flush();

        const shown = await waitFor(() =>
            view.container.textContent!.includes("Connect GitHub")
        );
        expect(shown).toBe(true);
        view.unmount();
    });

    it("lists threads grouped by repo with humanized types", async () => {
        const services = backendWith([
            {
                notifications: [
                    thread(),
                    thread({
                        id: "2",
                        unread: false,
                        reason: "subscribed",
                        subjectTitle: "Docs update",
                        subjectType: "Issue",
                        htmlUrl: null,
                    }),
                ],
                page: 1,
                hasMore: false,
            },
        ]);
        const view = renderWith(services, <InboxPage />);
        await flush();

        const listed = await waitFor(() =>
            view.container.textContent!.includes("Fix the bug")
        );
        expect(listed).toBe(true);
        expect(view.container.textContent).toContain("octocat/repo");
        expect(view.container.textContent).toContain("Docs update");
        expect(view.container.textContent).toContain("Pull request");
        expect(view.container.textContent).not.toContain("Mention");
        expect(view.container.textContent).not.toContain("Load more");
        const panel = view.container.querySelector(
            '[data-testid="inbox-panel"]'
        )!;
        expect(
            panel.querySelector('[aria-label="Mark Docs update read"]')
        ).toBeNull();
        view.unmount();
    });

    it("marks one thread read through the backend", async () => {
        const services = backendWith([
            { notifications: [thread()], page: 1, hasMore: false },
        ]);
        const view = renderWith(services, <InboxPage />);
        await flush();
        await waitFor(() =>
            view.container.textContent!.includes("Fix the bug")
        );

        const button = view.container.querySelector(
            '[aria-label="Mark Fix the bug read"]'
        )!;
        await click(button);

        const marked = await waitFor(
            () => services.markReadCalls().length === 1
        );
        expect(marked).toBe(true);
        expect(services.markReadCalls()).toEqual(["1"]);
        view.unmount();
    });

    it("resolves release links through the backend on click", async () => {
        const services = backendWith([
            {
                notifications: [
                    thread({
                        id: "7",
                        subjectTitle: "v2.0",
                        subjectType: "Release",
                        htmlUrl: null,
                        subjectUrl:
                            "https://api.github.com/repos/octocat/repo/releases/7",
                    }),
                ],
                page: 1,
                hasMore: false,
            },
        ]);
        const view = renderWith(services, <InboxPage />);
        await flush();
        await waitFor(() => view.container.textContent!.includes("v2.0"));

        const titleButton = [...view.container.querySelectorAll("button")].find(
            (element) => element.textContent === "v2.0"
        )!;
        await click(titleButton);

        const resolved = await waitFor(
            () => services.resolveCalls().length === 1
        );
        expect(resolved).toBe(true);
        expect(services.resolveCalls()).toEqual([
            "https://api.github.com/repos/octocat/repo/releases/7",
        ]);
        view.unmount();
    });

    it("loads the next page on demand", async () => {
        const services = backendWith([
            { notifications: [thread()], page: 1, hasMore: true },
            {
                notifications: [
                    thread({ id: "3", subjectTitle: "Second page item" }),
                ],
                page: 2,
                hasMore: false,
            },
        ]);
        const view = renderWith(services, <InboxPage />);
        await flush();
        await waitFor(() =>
            view.container.textContent!.includes("Fix the bug")
        );

        const loadMore = [...view.container.querySelectorAll("button")].find(
            (element) => element.textContent === "Load more"
        )!;
        await click(loadMore);

        const second = await waitFor(() =>
            view.container.textContent!.includes("Second page item")
        );
        expect(second).toBe(true);
        view.unmount();
    });

    it("shows an empty state when there is nothing to read", async () => {
        const services = backendWith([
            { notifications: [], page: 1, hasMore: false },
        ]);
        const view = renderWith(services, <InboxPage />);
        await flush();

        const empty = await waitFor(() =>
            view.container.textContent!.includes("All caught up")
        );
        expect(empty).toBe(true);
        view.unmount();
    });
});
