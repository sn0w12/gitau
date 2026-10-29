// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it } from "vitest";

import { RepoViewSwitcher } from "@/components/repo/history/view-switcher";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    AccountProfile,
    Capabilities,
    ConnectionProfile,
    ProviderDescriptor,
    RemoteInfo,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const REPO_ID = 1;

const PROFILE: AccountProfile = {
    login: "octocat",
    name: null,
    avatarUrl: "",
    htmlUrl: "",
    scopes: [],
    connectedAtMs: 0,
};

function capabilities(overrides: Partial<Capabilities> = {}): Capabilities {
    return {
        issues: true,
        pullRequests: true,
        reviews: true,
        notifications: true,
        checks: true,
        commitStatuses: true,
        workflowRuns: true,
        issuesWrite: true,
        pullRequestsWrite: true,
        publish: true,
        orgs: true,
        ...overrides,
    };
}

function backendWith(caps: Capabilities) {
    const provider: ProviderDescriptor = {
        id: "github",
        label: "GitHub",
        hosts: ["github.com"],
        imageHosts: [],
        capabilities: caps,
    };
    const connection: ConnectionProfile = {
        provider: "github",
        host: "github.com",
        account: PROFILE,
    };
    const backend = {
        forge: {
            providers: async (): Promise<Result<ProviderDescriptor[]>> => ({
                ok: true,
                value: [provider],
            }),
            connections: async (): Promise<Result<ConnectionProfile[]>> => ({
                ok: true,
                value: [connection],
            }),
        },
        remotes: {
            list: async (): Promise<Result<RemoteInfo[]>> => ({
                ok: true,
                value: [
                    {
                        name: "origin",
                        url: "https://github.com/octocat/repo.git",
                    },
                ],
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

async function waitFor(predicate: () => boolean): Promise<boolean> {
    const deadline = Date.now() + 1_000;
    while (Date.now() < deadline) {
        if (predicate()) return true;
        await new Promise((resolve) => setTimeout(resolve, 25));
    }
    return predicate();
}

async function openMenu(
    ownerDocument: Document,
    trigger: Element
): Promise<HTMLElement> {
    await click(trigger);
    const opened = await waitFor(
        () => ownerDocument.querySelector('[data-slot="menu-popup"]') !== null
    );
    expect(opened).toBe(true);
    return ownerDocument.querySelector(
        '[data-slot="menu-popup"]'
    ) as HTMLElement;
}

function menuItems(popup: HTMLElement): HTMLElement[] {
    return [
        ...popup.querySelectorAll(
            '[data-slot="menu-item"], [data-slot="menu-radio-item"]'
        ),
    ] as HTMLElement[];
}

describe("RepoViewSwitcher", () => {
    beforeEach(() => {
        seedSettingsForTests({
            switchOverviewViewShortcut: "Mod+O",
            switchGraphViewShortcut: "Mod+G",
            switchIssuesViewShortcut: "Mod+I",
            switchPullsViewShortcut: "Mod+P",
        });
        document.body.innerHTML = "";
    });

    it("shows the active view and switches on click", async () => {
        const services = backendWith(capabilities());
        const picked: string[] = [];
        const view = renderWith(
            services,
            <RepoViewSwitcher
                repoId={REPO_ID}
                view="overview"
                onView={(next) => picked.push(next)}
            />
        );
        await flush();

        expect(view.container.textContent).toContain("Current View");
        expect(view.container.textContent).toContain("Overview");

        const ownerDocument = view.container.ownerDocument;
        const trigger = view.container.querySelector(
            '[data-slot="menu-trigger"]'
        ) as HTMLElement;

        const popup = await openMenu(ownerDocument, trigger);
        expect(popup.textContent).toContain("Commit Graph");
        expect(popup.textContent).toContain("Issues");
        expect(popup.textContent).toContain("Pull requests");

        const graphItem = menuItems(popup).find((item) =>
            item.textContent?.includes("Commit Graph")
        );
        expect(graphItem).toBeDefined();
        await click(graphItem as HTMLElement);
        await flush();
        expect(picked).toEqual(["graph"]);

        const issuePopup = await openMenu(ownerDocument, trigger);
        const issueItem = menuItems(issuePopup).find((item) =>
            item.textContent?.includes("Issues")
        );
        expect(issueItem?.getAttribute("aria-disabled")).toBe(null);
        await click(issueItem as HTMLElement);
        await flush();
        expect(picked).toEqual(["graph", "issues"]);

        const prPopup = await openMenu(ownerDocument, trigger);
        const prItem = menuItems(prPopup).find((item) =>
            item.textContent?.includes("Pull requests")
        );
        expect(prItem?.getAttribute("aria-disabled")).toBe(null);
        await click(prItem as HTMLElement);
        await flush();
        expect(picked).toEqual(["graph", "issues", "pulls"]);
        view.unmount();
    });

    it("disables views the provider cannot serve instead of hiding them", async () => {
        const services = backendWith(
            capabilities({ issues: false, pullRequests: false })
        );
        const picked: string[] = [];
        const view = renderWith(
            services,
            <RepoViewSwitcher
                repoId={REPO_ID}
                view="overview"
                onView={(next) => picked.push(next)}
            />
        );
        await flush();

        const popup = await openMenu(
            view.container.ownerDocument,
            view.container.querySelector('[data-slot="menu-trigger"]')!
        );
        const items = menuItems(popup);
        expect(items.map((item) => item.textContent!.trim())).toHaveLength(4);

        const issues = items.find((item) =>
            item.textContent?.includes("Issues")
        );
        const pulls = items.find((item) =>
            item.textContent?.includes("Pull requests")
        );
        const graph = items.find((item) =>
            item.textContent?.includes("Commit Graph")
        );
        expect(issues?.getAttribute("data-disabled")).not.toBeNull();
        expect(pulls?.getAttribute("data-disabled")).not.toBeNull();
        expect(graph?.getAttribute("data-disabled")).toBeNull();

        await click(issues as HTMLElement);
        await flush();
        expect(picked).toEqual([]);
        view.unmount();
    });
});
