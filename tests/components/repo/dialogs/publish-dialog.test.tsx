// @vitest-environment jsdom
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it } from "vitest";

import { PublishRepositoryDialog } from "@/components/repo/dialogs/publish-dialog";
import { ToastProvider } from "@/components/ui/toast";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    AccountProfile,
    Capabilities,
    Org,
    ProviderDescriptor,
    PublishResult,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import type { Result } from "@/lib/backend/transport/result";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const PROFILE: AccountProfile = {
    login: "octocat",
    name: null,
    avatarUrl: "",
    htmlUrl: "",
    scopes: ["repo", "read:user", "workflow"],
    connectedAtMs: 1,
};

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

const GITLAB_PROVIDER: ProviderDescriptor = {
    id: "gitlab",
    label: "GitLab",
    hosts: ["gitlab.com"],
    imageHosts: [],
    capabilities: CAPABILITIES,
};

const GITLAB_PROFILE: AccountProfile = {
    ...PROFILE,
    login: "tanuki",
};

/** Owners each provider's account can publish under. */
const ORGS: Record<string, string[]> = {
    github: ["acme-corp"],
    gitlab: ["tanuki-group"],
};

function backendWith(
    options: { publish?: boolean; secondProvider?: boolean } = {}
) {
    let publishOutcome: Result<PublishResult> | null = null;
    const publishCalls: Record<string, unknown>[] = [];
    const provider =
        options.publish === false
            ? {
                  ...PROVIDER,
                  capabilities: { ...CAPABILITIES, publish: false },
              }
            : PROVIDER;

    const backend = {
        forge: {
            providers: async (): Promise<Result<ProviderDescriptor[]>> => ({
                ok: true,
                value: options.secondProvider
                    ? [provider, GITLAB_PROVIDER]
                    : [provider],
            }),
            account: async (
                id: string
            ): Promise<Result<AccountProfile | null>> => ({
                ok: true,
                value: id === "gitlab" ? GITLAB_PROFILE : PROFILE,
            }),
            listOrgs: async (id: string): Promise<Result<Org[]>> => ({
                ok: true,
                value: (ORGS[id] ?? []).map((login) => ({
                    login,
                    avatarUrl: undefined,
                })),
            }),
            publishRepository: async (
                _repoId: number,
                options: Record<string, unknown>
            ): Promise<Result<PublishResult>> => {
                publishCalls.push(options);
                if (!publishOutcome) throw new Error("no scripted outcome");
                return publishOutcome;
            },
        },
    };

    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        setPublishOutcome(next: Result<PublishResult>) {
            publishOutcome = next;
        },
        publishCalls,
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

async function typeInto(
    element: HTMLInputElement,
    value: string
): Promise<void> {
    // Bypass React's value tracker: assign via the native setter so the
    // dispatched input event registers as a change.
    const descriptor = Object.getOwnPropertyDescriptor(
        window.HTMLInputElement.prototype,
        "value"
    )!;
    descriptor.set!.call(element, value);
    await act(async () => {
        element.dispatchEvent(new Event("input", { bubbles: true }));
        await new Promise((resolve) => setTimeout(resolve, 10));
    });
}

async function clickButtonWithText(text: string): Promise<void> {
    await flush();
    const button = [...document.body.querySelectorAll("button")].find(
        (candidate) => candidate.textContent!.includes(text)
    );
    if (!button) throw new Error(`button not found: ${text}`);
    await act(async () => {
        button.dispatchEvent(
            new MouseEvent("click", { bubbles: true, cancelable: true })
        );
        await new Promise((resolve) => setTimeout(resolve, 30));
    });
}

async function waitForElement<T extends Element>(selector: string): Promise<T> {
    const deadline = Date.now() + 2_000;
    while (Date.now() < deadline) {
        const element = document.body.querySelector<T>(selector);
        if (element) return element;
        await new Promise((resolve) => setTimeout(resolve, 25));
    }
    const element = document.body.querySelector<T>(selector);
    if (!element) throw new Error(`element not found: ${selector}`);
    return element;
}

/** Resolves the control a FieldLabel points at through its `htmlFor`. */
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
    const element = labeledControl<T>(labelText);
    if (!element) throw new Error(`labeled control not found: ${labelText}`);
    return element;
}

async function clickElement(element: Element): Promise<void> {
    await act(async () => {
        element.dispatchEvent(
            new MouseEvent("click", { bubbles: true, cancelable: true })
        );
        await new Promise((resolve) => setTimeout(resolve, 30));
    });
}

async function selectItem(element: Element): Promise<void> {
    // Base UI commits an item on click only after a pointerdown marked the
    // interaction as a mouse selection.
    await act(async () => {
        element.dispatchEvent(
            new MouseEvent("pointerdown", { bubbles: true, cancelable: true })
        );
        element.dispatchEvent(
            new MouseEvent("click", { bubbles: true, cancelable: true })
        );
        await new Promise((resolve) => setTimeout(resolve, 30));
    });
}

async function openOwnerSelect(): Promise<HTMLElement> {
    const trigger = await waitForLabeledControl<HTMLElement>("Owner");
    await clickElement(trigger);
    return await waitForElement<HTMLElement>('[data-slot="select-popup"]');
}

const REPO_PATH = "C:/repos/demo";
const DIALOG_PROPS = {
    repoId: 5,
    repoPath: REPO_PATH,
    open: true,
    onClose: () => {},
};

describe("PublishRepositoryDialog", () => {
    beforeEach(() => {
        seedSettingsForTests({});
        document.body.innerHTML = "";
    });

    it("prefills the repo name and publishes with the form values", async () => {
        const services = backendWith();
        services.setPublishOutcome({
            ok: true,
            value: {
                fullName: "octocat/demo",
                htmlUrl: "https://github.com/octocat/demo",
                defaultBranch: "main",
            },
        });

        const view = renderWith(
            services,
            <PublishRepositoryDialog {...DIALOG_PROPS} />
        );
        await flush();

        // The folder name seeds the repository name.
        const nameInput = await waitForLabeledControl<HTMLInputElement>("Name");
        expect(nameInput.value).toBe("demo");

        const descriptionInput =
            await waitForLabeledControl<HTMLInputElement>("Description");
        await typeInto(nameInput, "demo-renamed");
        await typeInto(descriptionInput, "My demo project");

        await clickButtonWithText("Create repository");
        await flush();

        expect(services.publishCalls.length).toBe(1);
        expect(services.publishCalls[0]).toMatchObject({
            name: "demo-renamed",
            description: "My demo project",
            private: true,
            owner: undefined,
        });
        view.unmount();
    });

    it("says so when no provider can create repositories", async () => {
        const services = backendWith({ publish: false });
        const view = renderWith(
            services,
            <PublishRepositoryDialog {...DIALOG_PROPS} />
        );
        await flush();

        const notice = await waitForElement<HTMLElement>(
            "[data-testid=publish-dialog]"
        );
        expect(notice.textContent).toContain(
            "no provider that can create repositories"
        );
        const button = [...document.body.querySelectorAll("button")].find(
            (candidate) => candidate.textContent!.includes("Create repository")
        );
        expect(button?.disabled).toBe(true);
        expect(services.publishCalls.length).toBe(0);
        view.unmount();
    });

    it("groups owners by provider and publishes under the chosen one", async () => {
        const services = backendWith({ secondProvider: true });
        services.setPublishOutcome({
            ok: true,
            value: {
                fullName: "tanuki-group/demo",
                htmlUrl: "https://gitlab.com/tanuki-group/demo",
                defaultBranch: "main",
            },
        });

        const view = renderWith(
            services,
            <PublishRepositoryDialog {...DIALOG_PROPS} />
        );
        await flush();

        const popup = await openOwnerSelect();
        const groups = [
            ...popup.querySelectorAll('[data-slot="select-group"]'),
        ];
        expect(
            groups.map((group) =>
                group
                    .querySelector('[data-slot="select-group-label"]')!
                    .textContent!.trim()
            )
        ).toEqual(["GitHub", "GitLab"]);

        const gitlabOrg = [
            ...popup.querySelectorAll('[data-slot="select-item"]'),
        ].find((item) => item.textContent!.includes("tanuki-group"));
        expect(gitlabOrg).toBeDefined();
        await selectItem(gitlabOrg as Element);

        // The closed control names the provider too, since the owner alone is
        // ambiguous once two accounts are connected.
        expect(
            document.body.querySelector('[data-slot="select-value"]')!
                .textContent
        ).toContain("GitLab · tanuki-group");

        await clickButtonWithText("Create repository");
        await flush();

        expect(services.publishCalls[0]).toMatchObject({
            provider: "gitlab",
            owner: "tanuki-group",
        });
        view.unmount();
    });

    it("shows an inline conflict message when the name is taken", async () => {
        const services = backendWith();
        services.setPublishOutcome({
            ok: false,
            error: new GitBackendError({
                code: "forge",
                message: "github request failed (422)",
                detail: "name already exists on this account",
                retryable: false,
            }),
        });

        const view = renderWith(
            services,
            <PublishRepositoryDialog {...DIALOG_PROPS} />
        );
        await flush();

        await clickButtonWithText("Create repository");
        await flush();

        const alert = document.body.querySelector('[role="alert"]');
        expect(alert).not.toBeNull();
        expect(alert!.textContent).toContain("already taken");
        expect(services.publishCalls.length).toBe(1);
        view.unmount();
    });
});
