// @vitest-environment jsdom
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it } from "vitest";

import { ToastProvider } from "@/components/ui/toast";
import { ConfirmProvider } from "@/contexts/confirm-context";
import { AppServicesContext } from "@/contexts/services-context";
import type { AccountProfile, DeviceFlowStart } from "@/lib/backend/protocol";
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

const START: DeviceFlowStart = {
    userCode: "ABCD-1234",
    verificationUri: "https://github.com/login/device",
    expiresInSecs: 900,
};

function backendWith() {
    let accountValue: Result<AccountProfile | null> = {
        ok: true,
        value: null,
    };
    let completeOutcome: Result<AccountProfile> = {
        ok: false,
        error: new Error("not finished") as never,
    };
    let beginCalls = 0;
    let cancelCalls = 0;
    let signOutCalls = 0;
    let holdComplete = false;
    let releaseComplete: ((outcome: Result<AccountProfile>) => void) | null =
        null;

    const backend = {
        github: {
            account: async (): Promise<Result<AccountProfile | null>> =>
                accountValue,
            beginSignIn: async (): Promise<Result<DeviceFlowStart>> => {
                beginCalls += 1;
                return { ok: true, value: START };
            },
            completeSignIn: async (): Promise<Result<AccountProfile>> => {
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
        setCompleteOutcome(next: Result<AccountProfile>) {
            completeOutcome = next;
        },
        beginCalls: () => beginCalls,
        cancelCalls: () => cancelCalls,
        signOutCalls: () => signOutCalls,
        holdNextComplete() {
            holdComplete = true;
        },
        release(outcome: Result<AccountProfile>) {
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

describe("AccountPage", () => {
    beforeEach(() => {
        seedSettingsForTests({});
        document.body.innerHTML = "";
    });

    it("renders the signed-out state", async () => {
        const services = backendWith();
        const view = renderWith(services, <AccountPage />);
        await flush();

        expect(view.container.textContent).toContain("Sign in with GitHub");
        view.unmount();
    });

    it("runs the device flow: shows the code, then the connected card", async () => {
        const services = backendWith();
        services.holdNextComplete();
        const view = renderWith(services, <AccountPage />);
        await flush();

        await click(
            view.container.querySelector<HTMLButtonElement>(
                '[data-testid="account-panel"] button'
            )!
        );

        const codeShown = await waitFor(() =>
            view.container.textContent!.includes("ABCD-1234")
        );
        expect(codeShown).toBe(true);
        expect(services.beginCalls()).toBe(1);

        services.setAccount({ ok: true, value: profile() });
        act(() => {
            services.release({
                ok: true,
                value: profile(),
            });
        });

        const connected = await waitFor(() => {
            const el = view.container.querySelector(
                '[data-testid="github-account-connected"]'
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

        const card = view.container.querySelector(
            '[data-testid="github-account-connected"]'
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

        await click(
            view.container.querySelector<HTMLButtonElement>(
                '[data-testid="account-panel"] button'
            )!
        );
        await waitFor(() => view.container.textContent!.includes("ABCD-1234"));

        await click(
            view.container.querySelector(
                '[data-testid="github-cancel-signin"]'
            )!
        );

        const cancelled = await waitFor(() => services.cancelCalls() === 1);
        expect(cancelled).toBe(true);
        view.unmount();
    });
});
