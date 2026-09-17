import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { fireEvent } from "@testing-library/react";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CommitContextMenu } from "@/components/repo/history/commit-context-menu";
import { ToastProvider } from "@/components/ui/toast";
import { ConfirmProvider } from "@/contexts/confirm-context";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    CommitSummary,
    HeadState,
    RepoSnapshot,
    TagInfo,
    WorkflowOutcome,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import type { AppServices } from "@/lib/bootstrap/app-runtime";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const HEAD_OID = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const MERGE_PARENT_1 = "1111111111111111111111111111111111111111";
const MERGE_PARENT_2 = "2222222222222222222222222222222222222222";

function commit(overrides: Partial<CommitSummary> = {}): CommitSummary {
    return {
        id: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b",
        treeId: "tree",
        parentIds: ["4444444444444444444444444444444444444444"],
        summaryLine: "Add history prefetching",
        message: "Add history prefetching",
        author: {
            name: "Ada",
            email: "ada@example.com",
            timeSeconds: 1_700_000_000,
            timeOffsetMinutes: 0,
        },
        committer: {
            name: "Ada",
            email: "ada@example.com",
            timeSeconds: 1_700_000_000,
            timeOffsetMinutes: 0,
        },
        filesChanged: 2,
        additions: 10,
        deletions: 4,
        tags: [],
        ...overrides,
    };
}

function fakeBackend(head: HeadState) {
    const amendCalls: Array<{ message: string }> = [];
    const revertCalls: Array<{ target: string; parentIndex?: number }> = [];
    const createTagCalls: Array<{
        name: string;
        target?: string;
        message?: string;
    }> = [];
    const resetCalls: Array<{ kind: string; target: string }> = [];

    const snapshotResult: Result<RepoSnapshot> = {
        ok: true,
        value: {
            id: 5,
            generation: 1,
            gitDir: ".git",
            head,
            shaKind: "sha1",
        },
    };

    const backend = {
        repositories: {
            snapshot: async (): Promise<Result<RepoSnapshot>> => snapshotResult,
        },
        mutations: {
            amend: async (
                _repoId: number,
                message: string
            ): Promise<Result<CommitSummary>> => {
                amendCalls.push({ message });
                return { ok: true, value: commit() };
            },
            reset: async (
                _repoId: number,
                kind: string,
                target: string
            ): Promise<Result<void>> => {
                resetCalls.push({ kind, target });
                return { ok: true, value: undefined };
            },
        },
        workflows: {
            revert: async (
                _repoId: number,
                target: string,
                parentIndex?: number
            ): Promise<Result<WorkflowOutcome>> => {
                revertCalls.push({ target, parentIndex });
                return {
                    ok: true,
                    value: { outcome: "finished", commit: "abc123" },
                };
            },
        },
        refs: {
            createTag: async (
                _repoId: number,
                name: string,
                target?: string,
                message?: string
            ): Promise<Result<TagInfo>> => {
                createTagCalls.push({ name, target, message });
                return { ok: true, value: { name, target: target ?? "" } };
            },
        },
    };

    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        amendCalls,
        revertCalls,
        createTagCalls,
        resetCalls,
    };
}

type ServicesWithRecording = ReturnType<typeof fakeBackend>;

function RowHarness({
    services,
    commit: rowCommit,
}: {
    services: AppServices;
    commit: CommitSummary;
}): ReactElement {
    return (
        <AppServicesContext.Provider value={services}>
            <QueryClientProvider client={services.queryClient}>
                <ToastProvider>
                    <ConfirmProvider>
                        <ul>
                            <CommitContextMenu
                                repoId={5}
                                commit={rowCommit}
                                render={
                                    <li
                                        role="button"
                                        tabIndex={0}
                                        data-testid="commit-row"
                                    >
                                        {rowCommit.summaryLine}
                                    </li>
                                }
                            />
                        </ul>
                    </ConfirmProvider>
                </ToastProvider>
            </QueryClientProvider>
        </AppServicesContext.Provider>
    );
}

function renderWith(services: ServicesWithRecording, commit: CommitSummary) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(<RowHarness services={services} commit={commit} />);
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

/** Base UI portals attach outside the rendered container; poll the document. */
async function waitFor(predicate: () => boolean): Promise<boolean> {
    const deadline = Date.now() + 1_000;
    while (Date.now() < deadline) {
        if (predicate()) return true;
        await new Promise((resolve) => setTimeout(resolve, 25));
    }
    return predicate();
}

async function openMenu(view: ReturnType<typeof renderWith>) {
    const row = view.container.querySelector('[data-testid="commit-row"]');
    expect(row).not.toBeNull();
    await rightClick(row!);
    const opened = await waitFor(() =>
        Boolean(document.querySelector('[data-slot="context-menu-popup"]'))
    );
    expect(opened).toBe(true);
}

function menuItem(testId: string): Element {
    const item = document.querySelector(`[data-testid="${testId}"]`);
    expect(item).not.toBeNull();
    return item!;
}

function dialogButton(label: string): HTMLButtonElement {
    const popup = document.querySelector('[data-slot="dialog-popup"]');
    expect(popup).not.toBeNull();
    const button = Array.from(popup!.querySelectorAll("button")).find(
        (candidate) => candidate.textContent === label
    );
    expect(button).toBeDefined();
    return button!;
}

describe("CommitContextMenu", () => {
    const clipboard = { writeText: vi.fn().mockResolvedValue(undefined) };

    beforeEach(() => {
        seedSettingsForTests({});
        Object.defineProperty(navigator, "clipboard", {
            value: clipboard,
            configurable: true,
        });
        clipboard.writeText.mockClear();
        document.body.innerHTML = "";
    });

    afterEach(() => {
        document.body.innerHTML = "";
    });

    it("copies the full commit SHA", async () => {
        const rowCommit = commit();
        const services = fakeBackend({
            state: "attached",
            branch: "refs/heads/main",
            target: HEAD_OID,
        });
        const view = renderWith(services, rowCommit);

        await openMenu(view);
        await click(menuItem("commit-menu-copy-sha"));

        expect(clipboard.writeText).toHaveBeenCalledWith(rowCommit.id);
        view.unmount();
    });

    it("disables amend and undo for a commit that is not HEAD", async () => {
        const rowCommit = commit();
        const services = fakeBackend({
            state: "attached",
            branch: "refs/heads/main",
            target: HEAD_OID,
        });
        const view = renderWith(services, rowCommit);

        await openMenu(view);
        const amend = menuItem("commit-menu-amend");
        const undo = menuItem("commit-menu-undo");
        expect(
            amend.getAttribute("aria-disabled") === "true" ||
                amend.hasAttribute("data-disabled")
        ).toBe(true);
        expect(
            undo.getAttribute("aria-disabled") === "true" ||
                undo.hasAttribute("data-disabled")
        ).toBe(true);
        view.unmount();
    });

    it("amends the HEAD commit message through the dialog", async () => {
        const rowCommit = commit();
        const services = fakeBackend({
            state: "attached",
            branch: "refs/heads/main",
            target: rowCommit.id,
        });
        const view = renderWith(services, rowCommit);

        await openMenu(view);
        await click(menuItem("commit-menu-amend"));

        const opened = await waitFor(() =>
            Boolean(
                document.querySelector('[data-testid="amend-commit-dialog"]')
            )
        );
        expect(opened).toBe(true);

        const textarea = document.querySelector(
            "#amend-message"
        ) as HTMLTextAreaElement | null;
        expect(textarea).not.toBeNull();
        expect(textarea!.value).toBe(rowCommit.message);
        await act(async () => {
            fireEvent.change(textarea!, {
                target: { value: "Rewritten message" },
            });
        });

        const popup = document.querySelector('[data-slot="dialog-popup"]');
        const submit = Array.from(popup!.querySelectorAll("button")).find(
            (button) => button.textContent === "Amend commit"
        );
        expect(submit).toBeDefined();
        await click(submit!);

        const settled = await waitFor(() => services.amendCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.amendCalls[0]).toEqual({
            message: "Rewritten message",
        });
        view.unmount();
    });

    it("reverts a merge commit against its first parent without confirming", async () => {
        const rowCommit = commit({
            id: "3333333333333333333333333333333333333333",
            parentIds: [MERGE_PARENT_1, MERGE_PARENT_2],
            summaryLine: "Merge branch feature",
        });
        const services = fakeBackend({
            state: "attached",
            branch: "refs/heads/main",
            target: HEAD_OID,
        });
        const view = renderWith(services, rowCommit);

        await openMenu(view);
        await click(menuItem("commit-menu-revert"));

        const settled = await waitFor(() => services.revertCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.revertCalls[0]).toEqual({
            target: rowCommit.id,
            parentIndex: 1,
        });
        view.unmount();
    });

    it("creates a lightweight tag when the annotation stays empty", async () => {
        const rowCommit = commit();
        const services = fakeBackend({
            state: "attached",
            branch: "refs/heads/main",
            target: HEAD_OID,
        });
        const view = renderWith(services, rowCommit);

        await openMenu(view);
        await click(menuItem("commit-menu-create-tag"));

        const opened = await waitFor(() =>
            Boolean(document.querySelector('[data-testid="create-tag-dialog"]'))
        );
        expect(opened).toBe(true);

        const name = document.querySelector(
            "#create-tag-name"
        ) as HTMLInputElement | null;
        expect(name).not.toBeNull();
        await act(async () => {
            fireEvent.change(name!, { target: { value: "v1.0.0" } });
        });

        const popup = document.querySelector('[data-slot="dialog-popup"]');
        const submit = Array.from(popup!.querySelectorAll("button")).find(
            (button) => button.textContent === "Create tag"
        );
        expect(submit).toBeDefined();
        await click(submit!);

        const settled = await waitFor(() => services.createTagCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.createTagCalls[0]).toEqual({
            name: "v1.0.0",
            target: rowCommit.id,
            message: undefined,
        });
        view.unmount();
    });

    it("undoes the HEAD commit as a soft reset to its parent", async () => {
        const parent = "4444444444444444444444444444444444444444";
        const rowCommit = commit({ parentIds: [parent] });
        const services = fakeBackend({
            state: "attached",
            branch: "refs/heads/main",
            target: rowCommit.id,
        });
        const view = renderWith(services, rowCommit);

        await openMenu(view);
        await click(menuItem("commit-menu-undo"));

        const opened = await waitFor(() =>
            Boolean(document.querySelector('[data-slot="dialog-popup"]'))
        );
        expect(opened).toBe(true);
        await click(dialogButton("Undo commit"));

        const settled = await waitFor(() => services.resetCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.resetCalls[0]).toEqual({
            kind: "soft",
            target: parent,
        });
        view.unmount();
    });
});
