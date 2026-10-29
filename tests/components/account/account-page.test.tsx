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
    AuthChallenge,
    Capabilities,
    ConnectionProfile,
    ProviderDescriptor,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import { AccountPage } from "@/routes/account-page";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

function profile(overrides: Partial<AccountProfile> = {}): AccountProfile {
    return {
        login: "octocat",
        name: "The Octocat",
        avatarUrl: "https://avatars.githubusercontent.com/u/1",
        htmlUrl: "https://github.com/octocat",
        scopes: ["repo", "read:user", "workflow"],
        connectedAtMs: 1_700_000_000_000,
        ...overrides,
    };
}

const START: AuthChallenge = {
    userCode: "ABCD-1234",
    verificationUri: "https://github.com/login/device",
    expiresInSecs: 900,
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

const SECOND_PROVIDER: ProviderDescriptor = {
    id: "gitlab",
    label: "GitLab",
    hosts: ["gitlab.com"],
    imageHosts: [],
    capabilities: CAPABILITIES,
};

function connection(): ConnectionProfile {
    return { provider: "github", host: "github.com", account: profile() };
}

function backendWith(options: { providers?: ProviderDescriptor[] } = {}) {
    const providers = options.providers ?? [PROVIDER];
    let accountValue: Result<AccountProfile | null> = {
        ok: true,
        value: null,
    };
    let completeOutcome: Result<ConnectionProfile> = {
        ok: false,
        error: new Error("not finished") as never,
    };
    let beginCalls = 0;
    let cancelCalls = 0;
    let signOutCalls = 0;
    let holdComplete = false;
    let releaseComplete: ((outcome: Result<ConnectionProfile>) => void) | null =
        null;

    const backend = {
        forge: {
            providers: async (): Promise<Result<ProviderDescriptor[]>> => ({
                ok: true,
                value: providers,
            }),
            connections: async (): Promise<Result<ConnectionProfile[]>> =>
                accountValue.ok && accountValue.value
                    ? { ok: true, value: [connection()] }
                    : { ok: true, value: [] },
            account: async (): Promise<Result<AccountProfile | null>> =>
                accountValue,
            beginSignIn: async (): Promise<Result<AuthChallenge>> => {
                beginCalls += 1;
                return { ok: true, value: START };
            },
            completeSignIn: async (): Promise<Result<ConnectionProfile>> => {
                if (!holdComplete) return completeOutcome;
                return await new Promise((resolve) => {
                    releaseComplete = resolve;
                });
            },
            cancelSignIn: async (): Promise<Result<void>> => {
                cancelCalls += 1;
                return { ok: true, value: undefined };
            },
            signOut: async (): Promise<Result<void>> => {
                signOutCalls += 1;
                accountValue = { ok: true, value: null };
                return { ok: true, value: undefined };
            },
            listOrgs: async (): Promise<Result<{ login: string }[]>> => ({
                ok: true,
                value: [],
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
        setAccount(next: Result<AccountProfile | null>) {
            accountValue = next;
        },
        setCompleteOutcome(next: Result<ConnectionProfile>) {
            completeOutcome = next;
        },
        beginCalls: () => beginCalls,
        cancelCalls: () => cancelCalls,
        signOutCalls: () => signOutCalls,
        holdNextComplete() {
            holdComplete = true;
        },
        release(outcome: Result<ConnectionProfile>) {
            completeOutcome = outcome;
            releaseComplete?.(outcome);
            releaseComplete = null;
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

async function waitForButton(
    container: HTMLElement,
    text: string
): Promise<HTMLButtonElement> {
    await waitFor(() =>
        [...container.querySelectorAll("button")].some((candidate) =>
            candidate.textContent?.includes(text)
        )
    );
    const button = [...container.querySelectorAll("button")].find((candidate) =>
        candidate.textContent?.includes(text)
    );
    if (!button) throw new Error(`button not found: ${text}`);
    return button as HTMLButtonElement;
}

describe("AccountPage", () => {
    beforeEach(() => {
        seedSettingsForTests({});
        document.body.innerHTML = "";
    });

    it("renders the signed-out state", async () => {
        const services = backendWith();
        const view = renderWith(services, <AccountPage />);
        await flush();

        const shown = await waitFor(() =>
            view.container.textContent!.includes("Sign in with GitHub")
        );
        expect(shown).toBe(true);
        view.unmount();
    });

    it("renders a card per provider this build supports", async () => {
        const services = backendWith({
            providers: [PROVIDER, SECOND_PROVIDER],
        });
        const view = renderWith(services, <AccountPage />);
        await flush();

        const shown = await waitFor(
            () =>
                view.container.querySelector(
                    '[data-testid="provider-card-gitlab"]'
                ) != null
        );
        expect(shown).toBe(true);
        expect(
            view.container.querySelector('[data-testid="provider-card-github"]')
        ).not.toBeNull();
        view.unmount();
    });

    it("runs the device flow: shows the code, then the connected card", async () => {
        const services = backendWith();
        services.holdNextComplete();
        const view = renderWith(services, <AccountPage />);
        await flush();

        await click(await waitForButton(view.container, "Sign in with GitHub"));

        const codeShown = await waitFor(() =>
            view.container.textContent!.includes("ABCD-1234")
        );
        expect(codeShown).toBe(true);
        expect(services.beginCalls()).toBe(1);

        services.setAccount({ ok: true, value: profile() });
        act(() => {
            services.release({
                ok: true,
                value: connection(),
            });
        });

        const connected = await waitFor(() => {
            const el = view.container.querySelector(
                '[data-testid="forge-account-connected"]'
            );
            return el != null && el.textContent!.includes("octocat");
        });
        expect(connected).toBe(true);
        view.unmount();
    });

    it("shows profile details when already signed in", async () => {
        const services = backendWith();
        services.setAccount({ ok: true, value: profile() });
        const view = renderWith(services, <AccountPage />);
        await flush();

        const shown = await waitFor(
            () =>
                view.container.querySelector(
                    '[data-testid="forge-account-connected"]'
                ) != null
        );
        expect(shown).toBe(true);
        const card = view.container.querySelector(
            '[data-testid="forge-account-connected"]'
        );
        expect(card).not.toBeNull();
        expect(card!.textContent).toContain("The Octocat");
        expect(card!.textContent).toContain("Sign out");
        // Base UI renders the avatar image only once loaded (never in
        // jsdom), so assert the fallback initial instead.
        expect(card!.textContent).toContain("O");
        view.unmount();
    });

    it("cancels a pending sign-in through the backend", async () => {
        const services = backendWith();
        services.holdNextComplete();
        const view = renderWith(services, <AccountPage />);
        await flush();

        await click(await waitForButton(view.container, "Sign in with GitHub"));
        await waitFor(() => view.container.textContent!.includes("ABCD-1234"));

        await click(
            view.container.querySelector('[data-testid="forge-cancel-signin"]')!
        );

        const cancelled = await waitFor(() => services.cancelCalls() === 1);
        expect(cancelled).toBe(true);
        view.unmount();
    });
});
