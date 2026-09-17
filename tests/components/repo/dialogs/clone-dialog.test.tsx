// @vitest-environment jsdom
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { CloneRepoDialog } from "@/components/repo/dialogs/clone-dialog";
import { ToastProvider } from "@/components/ui/toast";
import { AppServicesContext } from "@/contexts/services-context";
import type { CloneEvent } from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import { seedSettingsForTests } from "@/stores/settings-store";

vi.mock("@tauri-apps/plugin-dialog", () => ({
    open: vi.fn(async () => "/repos"),
}));

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

function backendWith() {
    let onEvent: ((event: CloneEvent) => void) | null = null;
    const cloneInputs: Record<string, unknown>[] = [];
    const cancelCalls: number[] = [];

    const backend = {
        remotes: {
            clone: async (
                input: Record<string, unknown>,
                callback: (event: CloneEvent) => void
            ): Promise<Result<number>> => {
                cloneInputs.push(input);
                onEvent = callback;
                return { ok: true, value: 11 };
            },
        },
        operations: {
            cancel: async (operationId: number): Promise<Result<boolean>> => {
                cancelCalls.push(operationId);
                return { ok: true, value: true };
            },
        },
    };

    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        emit: (event: CloneEvent) => onEvent?.(event),
        cloneInputs,
        cancelCalls,
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

async function typeInto(selector: string, value: string): Promise<void> {
    const element = await waitFor<HTMLInputElement>(selector);
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

async function waitFor<T extends Element>(selector: string): Promise<T> {
    const deadline = Date.now() + 2_000;
    while (Date.now() < deadline) {
        const element = document.body.querySelector<T>(selector);
        if (element) return element;
        await new Promise((resolve) => setTimeout(resolve, 25));
    }
    throw new Error(`element not found: ${selector}`);
}

describe("CloneRepoDialog", () => {
    beforeEach(() => {
        seedSettingsForTests({ lastRepositoryDirectory: "" });
        document.body.innerHTML = "";
        vi.clearAllMocks();
    });

    it("derives the folder name from the url and streams progress in place", async () => {
        const services = backendWith();

        renderWith(
            services,
            <CloneRepoDialog open onClose={() => {}} onCloned={() => {}} />
        );
        await flush();

        await clickButtonWithText("Browse");
        await flush();
        await typeInto(
            'input[placeholder="https://github.com/owner/repo.git"]',
            "https://github.com/octocat/hello.git"
        );
        await flush();

        // Slug auto-filled from the URL.
        const folder = await waitFor<HTMLInputElement>(
            'input[placeholder="repo"]'
        );
        expect(folder.value).toBe("hello");

        await clickButtonWithText("Clone");

        // The same dialog now shows live transfer numbers.
        await waitFor('[data-testid="clone-phase"]');
        services.emit({
            event: "progress",
            operationId: 11,
            phase: "receiving",
            progress: 0.5,
            objectsReceived: 12,
            objectsTotal: 24,
            receivedBytes: 4096,
        });
        const objects = await waitFor('[data-testid="clone-objects"]');
        expect(objects.textContent).toContain("12 / 24");
        expect(services.cloneInputs[0]).toMatchObject({
            url: "https://github.com/octocat/hello.git",
            destination: "/repos/hello",
        });
    });

    it("cancel requests cancellation and returns to the form with a note", async () => {
        const services = backendWith();

        renderWith(
            services,
            <CloneRepoDialog open onClose={() => {}} onCloned={() => {}} />
        );
        await flush();

        await clickButtonWithText("Browse");
        await flush();
        await typeInto(
            'input[placeholder="https://github.com/owner/repo.git"]',
            "octocat/hello"
        );
        await clickButtonWithText("Clone");
        await waitFor('[data-testid="clone-phase"]');

        await clickButtonWithText("Cancel clone");
        expect(services.cancelCalls).toEqual([11]);

        services.emit({ event: "cancelled", operationId: 11 });
        const status = await waitFor('[role="status"]');
        expect(status.textContent).toContain("cancelled");
    });

    it("completes by invoking onCloned with the repo path", async () => {
        const services = backendWith();
        const onCloned = vi.fn();

        renderWith(
            services,
            <CloneRepoDialog open onClose={() => {}} onCloned={onCloned} />
        );
        await flush();

        await clickButtonWithText("Browse");
        await flush();
        await typeInto(
            'input[placeholder="https://github.com/owner/repo.git"]',
            "octocat/hello"
        );
        await clickButtonWithText("Clone");

        services.emit({
            event: "completed",
            operationId: 11,
            repoPath: "/repos/hello",
        });

        const deadline = Date.now() + 2_000;
        while (!onCloned.mock.calls.length && Date.now() < deadline) {
            await act(async () => {
                await new Promise((resolve) => setTimeout(resolve, 25));
            });
        }
        expect(onCloned).toHaveBeenCalledWith("/repos/hello");
    });
});
