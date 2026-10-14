import { QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";

import { RepoContextMenu } from "@/components/repo/repo-context-menu";
import { ToastProvider } from "@/components/ui/toast";
import { ConfirmProvider } from "@/contexts/confirm-context";
import { AppServicesContext } from "@/contexts/services-context";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import { resetAppRuntime } from "@/lib/bootstrap/app-runtime";
import { appStore, selectActiveTab } from "@/stores/app-store";
import { repositoryStore } from "@/stores/repository-store";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const REPO_PATH = "C:\\repos\\alpha";

function fakeServices() {
    const editorCalls: Array<{ path: string; relativePath?: string }> = [];
    const revealCalls: Array<{ path: string; relativePath?: string }> = [];
    const terminalCalls: string[] = [];
    const openCalls: string[] = [];
    const backend = {
        repositories: {
            open: (path: string): Promise<Result<{ id: number }>> => {
                openCalls.push(path);
                return Promise.resolve({ ok: true, value: { id: 7 } });
            },
        },
        editor: {
            openInEditor: (
                path: string,
                relativePath?: string
            ): Promise<Result<void>> => {
                editorCalls.push({ path, relativePath });
                return Promise.resolve({ ok: true, value: undefined });
            },
        },
        fileManager: {
            reveal: (
                path: string,
                relativePath?: string
            ): Promise<Result<void>> => {
                revealCalls.push({ path, relativePath });
                return Promise.resolve({ ok: true, value: undefined });
            },
        },
        terminal: {
            open: (path: string): Promise<Result<void>> => {
                terminalCalls.push(path);
                return Promise.resolve({ ok: true, value: undefined });
            },
        },
    };
    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        editorCalls,
        revealCalls,
        terminalCalls,
        openCalls,
    };
}

function renderWith(
    services: ReturnType<typeof fakeServices>,
    ui: ReactElement
) {
    resetAppRuntime(services);
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={services}>
                <ToastProvider>
                    <ConfirmProvider>{ui}</ConfirmProvider>
                </ToastProvider>
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

async function rightClick(element: Element): Promise<void> {
    await act(async () => {
        element.dispatchEvent(
            new MouseEvent("contextmenu", {
                bubbles: true,
                cancelable: true,
                clientX: 10,
                clientY: 10,
            })
        );
        await new Promise((resolve) => setTimeout(resolve, 10));
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

function menuItem(testId: string): Element {
    const item = document.querySelector(`[data-testid="${testId}"]`);
    expect(item).not.toBeNull();
    return item!;
}

describe("RepoContextMenu", () => {
    afterEach(() => {
        document.body.innerHTML = "";
        resetAppRuntime(null);
        appStore.setState(() => ({ tabs: [], activeTabId: null }));
        repositoryStore.setState(() => ({ entries: new Map() }));
    });

    it("opens the repo folder in the configured editor", async () => {
        seedSettingsForTests({ editorCommand: "code" });
        const services = fakeServices();
        const view = renderWith(
            services,
            <RepoContextMenu repoPath={REPO_PATH}>
                <button data-testid="repo-row">alpha</button>
            </RepoContextMenu>
        );

        await rightClick(
            view.container.querySelector('[data-testid="repo-row"]')!
        );
        const opened = await waitFor(() =>
            Boolean(document.querySelector('[data-slot="context-menu-popup"]'))
        );
        expect(opened).toBe(true);

        await click(menuItem("repo-menu-open-in-editor"));
        const settled = await waitFor(() => services.editorCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.editorCalls[0]).toEqual({
            path: REPO_PATH,
            relativePath: undefined,
        });
        view.unmount();
    });

    it("opens the repo folder in the default terminal without configuration", async () => {
        seedSettingsForTests({ editorCommand: "code" });
        const services = fakeServices();
        const view = renderWith(
            services,
            <RepoContextMenu repoPath={REPO_PATH}>
                <button data-testid="repo-row">alpha</button>
            </RepoContextMenu>
        );

        await rightClick(
            view.container.querySelector('[data-testid="repo-row"]')!
        );
        const opened = await waitFor(() =>
            Boolean(document.querySelector('[data-slot="context-menu-popup"]'))
        );
        expect(opened).toBe(true);

        const item = menuItem("repo-menu-open-in-terminal");
        expect(
            item.getAttribute("aria-disabled") === "true" ||
                item.hasAttribute("data-disabled")
        ).toBe(false);

        await click(item);
        const settled = await waitFor(() => services.terminalCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.terminalCalls[0]).toBe(REPO_PATH);
        view.unmount();
    });

    it("reveals the repo folder in the file manager", async () => {
        seedSettingsForTests({ editorCommand: "code" });
        const services = fakeServices();
        const view = renderWith(
            services,
            <RepoContextMenu repoPath={REPO_PATH}>
                <button data-testid="repo-row">alpha</button>
            </RepoContextMenu>
        );

        await rightClick(
            view.container.querySelector('[data-testid="repo-row"]')!
        );
        const opened = await waitFor(() =>
            Boolean(document.querySelector('[data-slot="context-menu-popup"]'))
        );
        expect(opened).toBe(true);

        await click(menuItem("repo-menu-reveal-in-file-manager"));
        const settled = await waitFor(() => services.revealCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.revealCalls[0]).toEqual({
            path: REPO_PATH,
            relativePath: undefined,
        });
        view.unmount();
    });

    it("disables the editor item until an editor command is set", async () => {
        seedSettingsForTests({ editorCommand: "" });
        const services = fakeServices();
        const view = renderWith(
            services,
            <RepoContextMenu repoPath={REPO_PATH}>
                <button data-testid="repo-row">alpha</button>
            </RepoContextMenu>
        );

        await rightClick(
            view.container.querySelector('[data-testid="repo-row"]')!
        );
        const opened = await waitFor(() =>
            Boolean(document.querySelector('[data-slot="context-menu-popup"]'))
        );
        expect(opened).toBe(true);

        const item = menuItem("repo-menu-open-in-editor");
        expect(
            item.getAttribute("aria-disabled") === "true" ||
                item.hasAttribute("data-disabled")
        ).toBe(true);
        expect(services.editorCalls).toHaveLength(0);
        view.unmount();
    });

    it("opens the repository in the active tab", async () => {
        seedSettingsForTests({ editorCommand: "code" });
        const services = fakeServices();
        const view = renderWith(
            services,
            <RepoContextMenu repoPath={REPO_PATH}>
                <button data-testid="repo-row">alpha</button>
            </RepoContextMenu>
        );

        await rightClick(
            view.container.querySelector('[data-testid="repo-row"]')!
        );
        const opened = await waitFor(() =>
            Boolean(document.querySelector('[data-slot="context-menu-popup"]'))
        );
        expect(opened).toBe(true);

        await click(menuItem("repo-menu-open"));
        const settled = await waitFor(() => services.openCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.openCalls[0]).toBe(REPO_PATH);
        view.unmount();
    });

    it("opens the repository in a new tab bound to this session's id", async () => {
        seedSettingsForTests({ editorCommand: "code" });
        const services = fakeServices();
        const view = renderWith(
            services,
            <RepoContextMenu repoPath={REPO_PATH}>
                <button data-testid="repo-row">alpha</button>
            </RepoContextMenu>
        );

        await rightClick(
            view.container.querySelector('[data-testid="repo-row"]')!
        );
        const opened = await waitFor(() =>
            Boolean(document.querySelector('[data-slot="context-menu-popup"]'))
        );
        expect(opened).toBe(true);

        await click(menuItem("repo-menu-open-in-new-tab"));
        const settled = await waitFor(() => appStore.state.tabs.length === 1);
        expect(settled).toBe(true);

        const tab = selectActiveTab();
        expect(tab?.repoPath).toBe(REPO_PATH);
        expect(tab?.lastResolvedHref).toBe("/repo/7");
        view.unmount();
    });
});
