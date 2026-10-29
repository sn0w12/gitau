// @vitest-environment jsdom
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it } from "vitest";

import { CreatePullDialog } from "@/components/repo/pulls/create-pull-dialog";
import { ToastProvider } from "@/components/ui/toast";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    AccountProfile,
    BranchInfo,
    Capabilities,
    CreatePullRequestBody,
    Label,
    ProviderDescriptor,
    PullRequestDetail,
    RemoteInfo,
    RepoListing,
    RepoSnapshot,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const CAPABILITIES: Capabilities = {
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
};

const PROVIDER: ProviderDescriptor = {
    id: "github",
    label: "GitHub",
    hosts: ["github.com"],
    imageHosts: [],
    capabilities: CAPABILITIES,
};

const PROFILE: AccountProfile = {
    login: "octocat",
    name: null,
    avatarUrl: "",
    htmlUrl: "",
    scopes: ["repo"],
    connectedAtMs: 1,
};

function branch(overrides: Partial<BranchInfo> & { name: string }): BranchInfo {
    return {
        target: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b",
        isHead: false,
        ...overrides,
    };
}

const LISTING: RepoListing = {
    branches: [
        branch({
            name: "main",
            isHead: true,
            upstream: { remote: "origin", branch: "main", ahead: 0, behind: 0 },
        }),
        branch({ name: "feature/thing" }),
    ],
    tags: [],
};

function listingWith(featurePublished: boolean): RepoListing {
    return {
        branches: LISTING.branches.map((entry) =>
            entry.name === "feature/thing" && featurePublished
                ? {
                      ...entry,
                      upstream: {
                          remote: "origin",
                          branch: "feature/thing",
                          ahead: 0,
                          behind: 0,
                      },
                  }
                : entry
        ),
        tags: [],
    };
}

const REMOTES: RemoteInfo[] = [
    { name: "origin", url: "https://github.com/octo/repo.git" },
];

const PULL = {
    number: 12,
    htmlUrl: "https://github.com/octo/repo/pull/12",
} as unknown as PullRequestDetail;

function backendWith(
    options: { pullRequestsWrite?: boolean; featurePublished?: boolean } = {}
) {
    const createCalls: {
        provider: string;
        owner: string;
        repo: string;
        body: CreatePullRequestBody;
    }[] = [];
    const pushCalls: Record<string, unknown>[] = [];
    const provider = {
        ...PROVIDER,
        capabilities: {
            ...CAPABILITIES,
            pullRequestsWrite: options.pullRequestsWrite ?? true,
        },
    };
    const snapshot: Result<RepoSnapshot> = {
        ok: true,
        value: {
            id: 5,
            generation: 1,
            gitDir: "/tmp/.git",
            head: { state: "attached", branch: "feature/thing", target: "abc" },
            shaKind: "sha1",
        },
    };
    const backend = {
        repositories: {
            snapshot: async () => snapshot,
            remotes: async (): Promise<Result<RemoteInfo[]>> => ({
                ok: true,
                value: REMOTES,
            }),
        },
        refs: {
            listBranchesAndTags: async (): Promise<Result<RepoListing>> => ({
                ok: true,
                value: listingWith(options.featurePublished ?? false),
            }),
        },
        remotes: {
            list: async (): Promise<Result<RemoteInfo[]>> => ({
                ok: true,
                value: REMOTES,
            }),
            push: async (
                _repoId: number,
                pushOptions: Record<string, unknown>
            ): Promise<Result<unknown[]>> => {
                pushCalls.push(pushOptions);
                return {
                    ok: true,
                    value: [
                        {
                            reference: "refs/heads/feature/thing",
                            accepted: true,
                        },
                    ],
                };
            },
        },
        forge: {
            providers: async (): Promise<Result<ProviderDescriptor[]>> => ({
                ok: true,
                value: [provider],
            }),
            account: async (): Promise<Result<AccountProfile | null>> => ({
                ok: true,
                value: PROFILE,
            }),
            listLabels: async (): Promise<Result<Label[]>> => ({
                ok: true,
                value: [{ name: "bug", color: "d73a4a" }],
            }),
            createPull: async (
                providerId: string,
                owner: string,
                repo: string,
                body: CreatePullRequestBody
            ): Promise<Result<PullRequestDetail>> => {
                createCalls.push({
                    provider: providerId,
                    owner,
                    repo,
                    body,
                });
                return { ok: true, value: PULL };
            },
        },
    };
    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        createCalls,
        pushCalls,
    };
}

type Services = ReturnType<typeof backendWith>;

function renderWith(services: Services) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={services}>
                <QueryClientProvider client={services.queryClient}>
                    <ToastProvider>
                        <CreatePullDialog
                            repoId={5}
                            initialHead="feature/thing"
                            open
                            onClose={() => {}}
                        />
                    </ToastProvider>
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

async function flush(): Promise<void> {
    await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 20));
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

async function waitForElement<T extends Element>(selector: string): Promise<T> {
    const deadline = Date.now() + 2_000;
    while (Date.now() < deadline) {
        const element = document.querySelector<T>(selector);
        if (element) return element;
        await new Promise((resolve) => setTimeout(resolve, 25));
    }
    throw new Error(`element not found: ${selector}`);
}

function labeledControl<T extends HTMLElement>(labelText: string): T | null {
    const label = [...document.body.querySelectorAll("label")].find(
        (candidate) => candidate.textContent?.trim() === labelText
    );
    const id = label?.getAttribute("for");
    return id ? (document.getElementById(id) as T | null) : null;
}

async function waitForLabeledControl<T extends HTMLElement>(
    labelText: string
): Promise<T> {
    const deadline = Date.now() + 2_000;
    while (Date.now() < deadline) {
        const element = labeledControl<T>(labelText);
        if (element) return element;
        await new Promise((resolve) => setTimeout(resolve, 25));
    }
    throw new Error(`labeled control not found: ${labelText}`);
}

async function typeInto(
    element: HTMLInputElement | HTMLTextAreaElement,
    value: string
): Promise<void> {
    const prototype =
        element instanceof HTMLTextAreaElement
            ? HTMLTextAreaElement.prototype
            : HTMLInputElement.prototype;
    const setter = Object.getOwnPropertyDescriptor(prototype, "value")!.set!;
    setter.call(element, value);
    await act(async () => {
        element.dispatchEvent(new Event("input", { bubbles: true }));
        await new Promise((resolve) => setTimeout(resolve, 10));
    });
}

async function clickButtonWithText(text: string): Promise<void> {
    await flush();
    const button = [...document.body.querySelectorAll("button")].find(
        (candidate) => candidate.textContent?.trim() === text
    );
    if (!button) throw new Error(`button not found: ${text}`);
    await act(async () => {
        button.dispatchEvent(
            new MouseEvent("click", { bubbles: true, cancelable: true })
        );
        await new Promise((resolve) => setTimeout(resolve, 30));
    });
}

describe("CreatePullDialog", () => {
    beforeEach(() => {
        document.body.innerHTML = "";
    });

    it("pushes the head branch and opens the pull request", async () => {
        seedSettingsForTests({});
        const services = backendWith();
        const view = renderWith(services);
        await flush();

        const title = await waitForLabeledControl<HTMLInputElement>("Title");
        expect((title as HTMLInputElement).value).toBe("feature/thing");
        await typeInto(title, "Add the thing");

        const description =
            await waitForLabeledControl<HTMLTextAreaElement>("Description");
        await typeInto(description, "Does the thing.");

        await clickButtonWithText("Create pull request");

        expect(await waitFor(() => services.createCalls.length > 0)).toBe(true);
        expect(services.pushCalls[0]).toEqual({
            remote: "origin",
            refspecs: ["feature/thing"],
            setUpstream: true,
        });
        expect(services.createCalls[0]).toEqual({
            provider: "github",
            owner: "octo",
            repo: "repo",
            body: {
                title: "Add the thing",
                head: "feature/thing",
                base: "main",
                body: "Does the thing.",
                draft: false,
                labels: [],
            },
        });
        view.unmount();
    });

    it("attaches the picked labels", async () => {
        seedSettingsForTests({});
        const services = backendWith();
        const view = renderWith(services);
        await flush();

        const title = await waitForLabeledControl<HTMLInputElement>("Title");
        await typeInto(title, "Add the thing");

        const input = await waitForElement<HTMLInputElement>(
            'input[aria-label="Select labels"]'
        );
        await typeInto(input, "bug");
        await act(async () => {
            input.dispatchEvent(
                new KeyboardEvent("keydown", {
                    key: "ArrowDown",
                    bubbles: true,
                    cancelable: true,
                })
            );
            await new Promise((resolve) => setTimeout(resolve, 30));
        });

        const item = await waitForElement<HTMLElement>(
            '[data-slot="combobox-item"]'
        );
        await act(async () => {
            item.dispatchEvent(
                new MouseEvent("pointerdown", {
                    bubbles: true,
                    cancelable: true,
                })
            );
            item.dispatchEvent(
                new MouseEvent("click", { bubbles: true, cancelable: true })
            );
            await new Promise((resolve) => setTimeout(resolve, 30));
        });

        await clickButtonWithText("Create pull request");
        expect(await waitFor(() => services.createCalls.length > 0)).toBe(true);
        expect(services.createCalls[0].body.labels).toEqual(["bug"]);
        view.unmount();
    });

    it("skips the push when the branch is already published", async () => {
        seedSettingsForTests({});
        const services = backendWith({ featurePublished: true });
        const view = renderWith(services);
        await flush();

        const title = await waitForLabeledControl<HTMLInputElement>("Title");
        await typeInto(title, "Add the thing");
        await clickButtonWithText("Create pull request");
        expect(await waitFor(() => services.createCalls.length > 0)).toBe(true);
        expect(services.pushCalls).toHaveLength(0);
        view.unmount();
    });

    it("explains when the provider cannot open pull requests", async () => {
        seedSettingsForTests({});
        const services = backendWith({ pullRequestsWrite: false });
        const view = renderWith(services);
        await flush();

        const explained = await waitFor(() =>
            document.body.textContent?.includes(
                "GitHub cannot open pull requests from this build."
            )
        );
        expect(explained).toBe(true);
        expect(labeledControl("Title")).toBeNull();
        view.unmount();
    });
});
