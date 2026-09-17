// @vitest-environment jsdom
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it } from "vitest";

import { PublishToGitHubDialog } from "@/components/repo/dialogs/publish-dialog";
import { ToastProvider } from "@/components/ui/toast";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    AccountProfile,
    GithubOrg,
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

function backendWith() {
    let publishOutcome: Result<PublishResult> | null = null;
    const publishCalls: Record<string, unknown>[] = [];

    const backend = {
        github: {
            account: async (): Promise<Result<AccountProfile | null>> => ({
                ok: true,
                value: PROFILE,
            }),
            listOrgs: async (): Promise<Result<GithubOrg[]>> => ({
                ok: true,
                value: [{ login: "acme-corp", avatarUrl: undefined }],
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

const REPO_PATH = "C:/repos/demo";
const DIALOG_PROPS = {
    repoId: 5,
    repoPath: REPO_PATH,
    open: true,
    onClose: () => {},
};

describe("PublishToGitHubDialog", () => {
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
            <PublishToGitHubDialog {...DIALOG_PROPS} />
        );
        await flush();

        // The folder name seeds the repository name.
        const nameInput =
            await waitForElement<HTMLInputElement>("#publish-name");
        expect(nameInput.value).toBe("demo");

        const descriptionInput = await waitForElement<HTMLInputElement>(
            "#publish-description"
        );
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

    it("shows an inline conflict message when the name is taken", async () => {
        const services = backendWith();
        services.setPublishOutcome({
            ok: false,
            error: new GitBackendError({
                code: "github",
                message: "github request failed (422)",
                detail: "name already exists on this account",
                retryable: false,
            }),
        });

        const view = renderWith(
            services,
            <PublishToGitHubDialog {...DIALOG_PROPS} />
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
