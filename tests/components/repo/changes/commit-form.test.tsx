// @vitest-environment jsdom
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { CommitForm } from "@/components/repo/changes/commit-form";
import { ToastProvider, toastManager } from "@/components/ui/toast";
import { ConfirmProvider } from "@/contexts/confirm-context";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    CommitHookEvent,
    GitHook,
    HookRunResult,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import type { Result } from "@/lib/backend/transport/result";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const PRE_COMMIT: GitHook = {
    name: "pre-commit",
    path: "/repo/.git/hooks/pre-commit",
    executable: true,
};

function services(commit: Result<unknown>) {
    const backend = {
        hooks: {
            list: async (): Promise<Result<GitHook[]>> => ({
                ok: true,
                value: [PRE_COMMIT],
            }),
            runStreamed: async (): Promise<Result<HookRunResult>> => ({
                ok: true,
                value: {
                    hook: "pre-commit",
                    exitCode: 1,
                    success: true,
                    stdout: "",
                    stderr: "",
                    durationMs: 4,
                    lines: [],
                    styles: [],
                },
            }),
        },
        mutations: {
            commit: async (
                _repoId: number,
                _message: string,
                options: {
                    onHookEvent?: (event: CommitHookEvent) => void;
                } = {}
            ): Promise<Result<unknown>> => {
                options.onHookEvent?.({
                    type: "settled",
                    result: {
                        hook: "pre-commit",
                        exitCode: 5,
                        success: false,
                        stdout: "lint found problems",
                        stderr: "",
                        durationMs: 9,
                        lines: [{ text: "lint found problems", spans: [] }],
                        styles: [],
                    },
                });
                return commit;
            },
        },
    };
    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
    };
}

function renderForm(commit: Result<unknown>) {
    const context = services(commit);
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={context}>
                <QueryClientProvider client={context.queryClient}>
                    <ToastProvider>
                        <ConfirmProvider>
                            <CommitForm
                                repoId={7}
                                branch="main"
                                stagedCount={1}
                            />
                        </ConfirmProvider>
                    </ToastProvider>
                </QueryClientProvider>
            </AppServicesContext.Provider>
        );
    });
    return {
        container,
        context,
        unmount() {
            act(() => root.unmount());
            container.remove();
        },
    };
}

async function submitAndFail(): Promise<void> {
    await act(async () => {
        const input = document.querySelector<HTMLInputElement>(
            "#commit-summary-input"
        )!;
        const setter = Object.getOwnPropertyDescriptor(
            HTMLInputElement.prototype,
            "value"
        )!.set!;
        setter.call(input, "add a feature");
        input.dispatchEvent(new Event("input", { bubbles: true }));
    });
    await act(async () => {
        document
            .querySelector('button[type="submit"]')!
            .dispatchEvent(
                new MouseEvent("click", { bubbles: true, cancelable: true })
            );
        await new Promise((resolve) => setTimeout(resolve, 30));
    });
}

function actionButton(): HTMLButtonElement | null {
    return document.querySelector<HTMLButtonElement>(
        '[data-slot="toast-action"]'
    );
}

async function clickAction(): Promise<void> {
    await act(async () => {
        actionButton()!.dispatchEvent(
            new MouseEvent("click", { bubbles: true, cancelable: true })
        );
        await new Promise((resolve) => setTimeout(resolve, 30));
    });
}

describe("CommitForm", () => {
    beforeEach(() => {
        document.body.innerHTML = "";
        seedSettingsForTests({});
        toastManager.close();
    });

    it("offers the hook checker from the toast when a hook rejects the commit", async () => {
        const spy = vi.spyOn(toastManager, "add");
        const view = renderForm({
            ok: false,
            error: new GitBackendError({
                code: "hookFailed",
                message: "hook `pre-commit` failed",
                retryable: false,
                detail: "exit 5\nlint found problems",
            }),
        });

        await submitAndFail();

        const toast = spy.mock.calls[0][0];
        expect(toast.description).toBe("hook `pre-commit` failed");
        expect(toast.actionProps?.children).toBe("Show hooks");

        // The row the commit's own pre-commit produced is on screen, expanded,
        // so the action lands the reader on the output rather than a rerun.
        await clickAction();
        const row = [...document.querySelectorAll("li")].find((element) =>
            element.textContent?.includes("pre-commit")
        );
        expect(row?.textContent).toContain("lint found problems");
        view.unmount();
        spy.mockRestore();
    });

    it("leaves a non-hook commit failure without an action", async () => {
        const spy = vi.spyOn(toastManager, "add");
        const view = renderForm({
            ok: false,
            error: new GitBackendError({
                code: "invalidInput",
                message: "no staged changes",
                retryable: false,
            }),
        });

        await submitAndFail();

        expect(spy.mock.calls[0][0].actionProps).toBeUndefined();
        view.unmount();
        spy.mockRestore();
    });
});
