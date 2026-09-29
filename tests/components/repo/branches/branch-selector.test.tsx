import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { describe, expect, it } from "vitest";

import { BranchSelector } from "@/components/repo/branches/branch-selector";
import { ToastProvider } from "@/components/ui/toast";
import { ConfirmProvider } from "@/contexts/confirm-context";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    BranchInfo,
    OperationState,
    RepoListing,
    RepoSnapshot,
    WorkflowOutcome,
    WorktreeInfo,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { Result } from "@/lib/backend/transport/result";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

function branch(overrides: Partial<BranchInfo> & { name: string }): BranchInfo {
    return {
        target: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b",
        isHead: false,
        ...overrides,
    };
}

function tree(overrides: Partial<WorktreeInfo> & { name: string }) {
    return {
        path: `/repos/${overrides.name}`,
        isCurrent: false,
        isPrimary: false,
        isPrunable: false,
        ...overrides,
    } satisfies WorktreeInfo;
}

const attachedHead: RepoSnapshot["head"] = {
    state: "attached",
    branch: "main",
    target: "abc",
};

function listingBackend(
    listingResult: Result<RepoListing>,
    head: RepoSnapshot["head"] = attachedHead,
    trees: WorktreeInfo[] = []
) {
    const checkoutCalls: string[] = [];
    const createCalls: string[] = [];
    const worktreeCreateCalls: string[] = [];
    const mergeCalls: string[] = [];
    const removeWorktreeCalls: string[] = [];
    let listingFetches = 0;
    const operationState: Result<OperationState> = {
        ok: true,
        value: { kind: "none", conflictPaths: [], heads: [] },
    };
    const snapshot: Result<RepoSnapshot> = {
        ok: true,
        value: {
            id: 5,
            generation: 1,
            gitDir: "/tmp/.git",
            head,
            shaKind: "sha1",
        },
    };
    const backend = {
        repositories: {
            snapshot: async (): Promise<Result<RepoSnapshot>> => snapshot,
        },
        refs: {
            listBranchesAndTags: async (): Promise<Result<RepoListing>> => {
                listingFetches += 1;
                return listingResult;
            },
            createBranch: (
                _repoId: number,
                name: string
            ): Promise<Result<BranchInfo>> => {
                createCalls.push(name);
                return Promise.resolve({
                    ok: true,
                    value: branch({ name, isHead: true }),
                });
            },
        },
        worktrees: {
            list: async (): Promise<Result<WorktreeInfo[]>> => ({
                ok: true,
                value: trees,
            }),
            create: (
                _repoId: number,
                name: string
            ): Promise<Result<WorktreeInfo>> => {
                worktreeCreateCalls.push(name);
                return Promise.resolve({ ok: true, value: tree({ name }) });
            },
            remove: (_repoId: number, name: string): Promise<Result<void>> => {
                removeWorktreeCalls.push(name);
                return Promise.resolve({ ok: true, value: undefined });
            },
        },
        mutations: {
            checkout: (
                _repoId: number,
                target: string
            ): Promise<Result<void>> => {
                checkoutCalls.push(target);
                return Promise.resolve({ ok: true, value: undefined });
            },
        },
        workflows: {
            operationState: async (): Promise<Result<OperationState>> =>
                operationState,
            merge: (
                _repoId: number,
                target: string
            ): Promise<Result<WorkflowOutcome>> => {
                mergeCalls.push(target);
                return Promise.resolve({
                    ok: true,
                    value: { outcome: "merged", commit: "abc" },
                });
            },
            mergeContinue: async (): Promise<Result<WorkflowOutcome>> => ({
                ok: true,
                value: { outcome: "finished", commit: "abc" },
            }),
            mergeAbort: async (): Promise<Result<WorkflowOutcome>> => ({
                ok: true,
                value: { outcome: "aborted" },
            }),
            resolveConflict: async (): Promise<Result<void>> => ({
                ok: true,
                value: undefined,
            }),
        },
    };
    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        checkoutCalls,
        createCalls,
        worktreeCreateCalls,
        mergeCalls,
        removeWorktreeCalls,
        listingFetches: () => listingFetches,
    };
}

type Services = ReturnType<typeof listingBackend>;

/** Leftover portals from an earlier test would shadow queries; wipe them. */
function cleanLeftoverPortals(): void {
    document
        .querySelectorAll(
            '[data-slot="popover-positioner"], [data-slot="popover-popup"], [data-slot="dialog-viewport"]'
        )
        .forEach((node) => node.remove());
}

function renderWith(services: Services, ui: ReactElement) {
    cleanLeftoverPortals();
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

/**
 * React tracks input values through the native setter; assigning `.value`
 * directly would be deduped by its value tracker and never fire onChange.
 */
function setInputValue(input: HTMLInputElement, value: string): void {
    const setter = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value"
    )!.set!;
    setter.call(input, value);
}

async function typeInto(input: HTMLInputElement, value: string): Promise<void> {
    await act(async () => {
        setInputValue(input, value);
        input.dispatchEvent(new Event("input", { bubbles: true }));
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

const listing: Result<RepoListing> = {
    ok: true,
    value: {
        branches: [
            branch({ name: "feature/late" }),
            branch({
                name: "main",
                isHead: true,
                upstream: {
                    remote: "origin",
                    branch: "main",
                    ahead: 2,
                    behind: 0,
                },
            }),
            branch({ name: "develop" }),
        ],
        tags: [],
    },
};

async function openSelector(services: Services) {
    seedSettingsForTests({});
    const view = renderWith(services, <BranchSelector repoId={5} />);
    await flush();
    // The trigger shows the current branch.
    expect(view.container.textContent).toContain("Current Branch");
    await click(view.container.querySelector('[data-slot="popover-trigger"]')!);
    const opened = await waitFor(() =>
        Boolean(document.querySelector('[data-slot="command-item"]'))
    );
    expect(opened).toBe(true);
    return view;
}

function itemTexts(): string[] {
    return Array.from(
        document.querySelectorAll('[data-slot="command-item"]')
    ).map((item) => item.textContent);
}

/** The plus button closes the popover and mounts the create dialog. */
async function openCreateDialog(services: Services) {
    const plus = Array.from(document.querySelectorAll("button")).find(
        (button) =>
            button.getAttribute("aria-label") === "New branch or worktree"
    )!;
    expect(plus).toBeDefined();
    await click(plus);
    const opened = await waitFor(() =>
        Boolean(
            document.querySelector('[data-testid="create-checkout-dialog"]')
        )
    );
    expect(opened).toBe(true);
    expect(services.createCalls).toHaveLength(0);
}

function dialogButton(label: string): HTMLButtonElement {
    return Array.from(document.querySelectorAll("button")).find(
        (button) => button.textContent === label
    )!;
}

describe("BranchSelector", () => {
    it("groups branches into default and other", async () => {
        const services = listingBackend(listing);
        const view = await openSelector(services);

        const labels = Array.from(
            document.querySelectorAll('[data-slot="command-group-label"]')
        ).map((label) => label.textContent);
        expect(labels).toEqual(["Default branch", "Other"]);

        // Default group first, others alphabetical within their group.
        const names = itemTexts();
        expect(names[0]).toContain("main");
        expect(names[1]).toContain("develop");
        expect(names[2]).toContain("feature/late");
        view.unmount();
    });

    it("marks the checked-out branch as selected", async () => {
        const services = listingBackend(listing);
        const view = await openSelector(services);

        const currentItem = Array.from(
            document.querySelectorAll('[data-slot="command-item"]')
        ).find((item) => item.getAttribute("aria-selected") === "true");
        expect(currentItem?.textContent).toContain("main");
        view.unmount();
    });

    it("filters through the command's built-in search", async () => {
        const services = listingBackend(listing);
        const view = await openSelector(services);

        const input = Array.from(document.querySelectorAll("input")).find(
            (candidate) =>
                candidate.getAttribute("aria-label") === "Search branches"
        )!;
        await typeInto(input, "dev");
        await flush();

        const names = itemTexts().join("|");
        expect(names).toContain("develop");
        expect(names).not.toContain("feature/late");

        await typeInto(input, "zzz-nothing");
        await flush();
        expect(itemTexts()).toHaveLength(0);
        expect(document.body.textContent).toContain("No branches found.");
        view.unmount();
    });

    it("shows an upstream ahead count on the row", async () => {
        const services = listingBackend(listing);
        const view = await openSelector(services);

        const mainItem = Array.from(
            document.querySelectorAll('[data-slot="command-item"]')
        ).find((item) => item.textContent.includes("main"));
        expect(mainItem?.textContent).toContain("2");
        view.unmount();
    });

    it("shows a detached group when HEAD is detached", async () => {
        const services = listingBackend(
            {
                ok: true,
                value: {
                    branches: [branch({ name: "develop" })],
                    tags: [],
                },
            },
            { state: "detached", target: "deadbeefcafe0000" }
        );
        const view = await openSelector(services);

        const labels = Array.from(
            document.querySelectorAll('[data-slot="command-group-label"]')
        ).map((label) => label.textContent);
        expect(labels).toEqual(["Detached", "Other"]);
        expect(itemTexts().join("|")).toContain("deadbee");
        view.unmount();
    });

    it("checks out the picked branch", async () => {
        const services = listingBackend(listing);
        const view = await openSelector(services);

        const developRow = Array.from(
            document.querySelectorAll('[data-slot="command-item"]')
        ).find((item) => item.textContent.includes("develop"));
        expect(developRow).toBeDefined();
        await click(developRow!);

        const settled = await waitFor(() => services.checkoutCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.checkoutCalls[0]).toBe("develop");
        view.unmount();
    });

    it("creates and switches to a new branch from the dialog", async () => {
        const services = listingBackend(listing);
        const view = await openSelector(services);

        await openCreateDialog(services);
        const nameInput = document.querySelector<HTMLInputElement>(
            '[data-testid="create-checkout-dialog"] [aria-label="Name"]'
        )!;
        await typeInto(nameInput, "feature/new-thing");
        await flush();

        const createButton = dialogButton("Create branch");
        expect(createButton.disabled).toBe(false);
        await click(createButton);

        const settled = await waitFor(() => services.createCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.createCalls[0]).toBe("feature/new-thing");
        expect(services.worktreeCreateCalls).toHaveLength(0);
        view.unmount();
    });

    it("creates a worktree from the same dialog", async () => {
        const services = listingBackend(listing);
        const view = await openSelector(services);

        await openCreateDialog(services);
        await click(dialogButton("Worktree"));
        await flush();

        const nameInput = document.querySelector<HTMLInputElement>(
            '[data-testid="create-checkout-dialog"] [aria-label="Name"]'
        )!;
        await typeInto(nameInput, "scratch");
        await flush();

        const createButton = dialogButton("Create worktree");
        expect(createButton.disabled).toBe(false);
        await click(createButton);

        const settled = await waitFor(
            () => services.worktreeCreateCalls.length > 0
        );
        expect(settled).toBe(true);
        expect(services.worktreeCreateCalls[0]).toBe("scratch");
        expect(services.createCalls).toHaveLength(0);
        view.unmount();
    });

    it("rejects invalid or duplicate branch names", async () => {
        const services = listingBackend(listing);
        const view = await openSelector(services);

        await openCreateDialog(services);
        const nameInput = document.querySelector<HTMLInputElement>(
            '[data-testid="create-checkout-dialog"] [aria-label="Name"]'
        )!;

        for (const bad of ["has space", "main"]) {
            await typeInto(nameInput, bad);
            await flush();
            expect(dialogButton("Create branch").disabled).toBe(true);
        }

        expect(services.createCalls).toHaveLength(0);
        view.unmount();
    });

    it("gives linked worktrees their own group below the branches", async () => {
        const services = listingBackend(listing, attachedHead, [
            tree({ name: "main", isCurrent: true, isPrimary: true }),
            tree({ name: "scratch", branch: "develop" }),
        ]);
        const view = await openSelector(services);

        const labels = Array.from(
            document.querySelectorAll('[data-slot="command-group-label"]')
        ).map((label) => label.textContent);
        expect(labels).toEqual(["Default branch", "Other", "Worktrees"]);

        // The primary tree is this repo, so only the linked one is a row, and
        // the develop branch it holds is not repeated among the branches.
        const names = itemTexts();
        expect(names).toHaveLength(3);
        expect(names.filter((text) => text.startsWith("develop"))).toHaveLength(
            0
        );
        expect(names.filter((text) => text.startsWith("main"))).toHaveLength(1);
        view.unmount();
    });

    it("hides a branch row that a linked worktree already lists", async () => {
        const services = listingBackend(listing, attachedHead, [
            tree({ name: "scratch", branch: "develop" }),
        ]);
        const view = await openSelector(services);

        const developRow = Array.from(
            document.querySelectorAll('[data-slot="command-item"]')
        ).find((item) => item.textContent?.startsWith("develop"));
        expect(developRow).toBeUndefined();

        const scratchRow = Array.from(
            document.querySelectorAll('[data-slot="command-item"]')
        ).find((item) => item.textContent?.startsWith("scratch"))!;
        expect(scratchRow.textContent).toBe("scratch");
        expect(scratchRow.getAttribute("aria-disabled")).toBeNull();
        view.unmount();
    });

    it("keeps listing the worktree this tab is bound to", async () => {
        // Opening a linked worktree as its own repo makes it the session's
        // tree, so it must stay in the list instead of reading as a branch.
        const services = listingBackend(listing, attachedHead, [
            tree({ name: "main", isCurrent: true, isPrimary: true }),
            tree({
                name: "scratch",
                branch: "opencode/scratch",
                isCurrent: true,
            }),
        ]);
        const view = await openSelector(services);

        const scratchRow = Array.from(
            document.querySelectorAll('[data-slot="command-item"]')
        ).find((item) => item.textContent?.startsWith("scratch"));
        expect(scratchRow).toBeDefined();
        expect(scratchRow!.getAttribute("aria-selected")).toBe("true");
        view.unmount();
    });

    it("keeps a default branch listed when a worktree holds it", async () => {
        // master is the only default branch and a worktree holds it, so
        // dropping claimed branches everywhere would erase the group.
        const services = listingBackend(
            {
                ok: true,
                value: {
                    branches: [branch({ name: "master", isHead: true })],
                    tags: [],
                },
            },
            { state: "attached", branch: "master", target: "abc" },
            [
                tree({ name: "main", isCurrent: true, isPrimary: true }),
                tree({ name: "playful-cactus", branch: "master" }),
            ]
        );
        const view = await openSelector(services);

        const labels = Array.from(
            document.querySelectorAll('[data-slot="command-group-label"]')
        ).map((label) => label.textContent);
        expect(labels).toContain("Default branch");

        const masterRow = Array.from(
            document.querySelectorAll('[data-slot="command-item"]')
        ).find((item) => item.textContent?.startsWith("master"))!;
        expect(masterRow.textContent).toContain("in playful-cactus");
        expect(masterRow.getAttribute("aria-disabled")).toBe("true");
        view.unmount();
    });

    it("offers a way back to the parent repo from inside a worktree", async () => {
        // Standing in the linked tree, the primary entry describes the repo
        // the tab came from and is the only row that leads back to it.
        const services = listingBackend(listing, attachedHead, [
            tree({
                name: "main",
                isCurrent: true,
                isPrimary: true,
                path: "/repos/Akari",
            }),
            tree({
                name: "silent-moon",
                branch: "opencode/silent-moon",
                isCurrent: true,
            }),
        ]);
        const view = await openSelector(services);

        const rows = Array.from(
            document.querySelectorAll('[data-slot="command-item"]')
        ).map((item) => item.textContent);
        // Labelled with the repo directory name, not the backend's "main".
        expect(rows).toContain("Akari");
        expect(rows).toContain("silent-moon");
        // The branch the current tree holds is represented by that tree's row.
        expect(
            rows.filter((text) => text.startsWith("opencode/silent-moon"))
        ).toHaveLength(0);
        view.unmount();
    });

    it("omits the way-back row when not inside a linked worktree", async () => {
        const services = listingBackend(listing, attachedHead, [
            tree({ name: "main", isCurrent: true, isPrimary: true }),
            tree({ name: "scratch", branch: "develop" }),
        ]);
        const view = await openSelector(services);

        const rows = Array.from(
            document.querySelectorAll('[data-slot="command-item"]')
        ).map((item) => item.textContent);
        expect(rows).toContain("scratch");
        // The primary tree is not offered, so no row is labelled with its
        // directory name. The branch row reads "main2" with its upstream count.
        expect(rows.filter((text) => text === "main")).toHaveLength(0);
        expect(rows).toHaveLength(3);
        view.unmount();
    });

    it("removes a worktree from its row context menu", async () => {
        const services = listingBackend(listing, attachedHead, [
            tree({ name: "scratch", branch: "develop" }),
        ]);
        const view = await openSelector(services);

        const row = Array.from(
            document.querySelectorAll('[data-slot="command-item"]')
        ).find((item) => item.textContent?.startsWith("scratch"))!;
        await act(async () => {
            row.dispatchEvent(
                new MouseEvent("contextmenu", {
                    bubbles: true,
                    cancelable: true,
                    clientX: 10,
                    clientY: 10,
                })
            );
            await new Promise((resolve) => setTimeout(resolve, 10));
        });

        const opened = await waitFor(() =>
            Array.from(
                document.querySelectorAll('[data-slot="context-menu-item"]')
            ).some((item) => item.textContent.includes("Remove worktree"))
        );
        expect(opened).toBe(true);

        await click(
            Array.from(
                document.querySelectorAll('[data-slot="context-menu-item"]')
            ).find((item) => item.textContent.includes("Remove worktree"))!
        );
        await click(dialogButton("Remove"));

        const settled = await waitFor(
            () => services.removeWorktreeCalls.length > 0
        );
        expect(settled).toBe(true);
        expect(services.removeWorktreeCalls[0]).toBe("scratch");
        view.unmount();
    });

    it("merges from the row context menu", async () => {
        const services = listingBackend(listing);
        const view = await openSelector(services);

        const developRow = Array.from(
            document.querySelectorAll('[data-slot="command-item"]')
        ).find((item) => item.textContent.includes("develop"))!;
        await act(async () => {
            developRow.dispatchEvent(
                new MouseEvent("contextmenu", {
                    bubbles: true,
                    cancelable: true,
                    clientX: 10,
                    clientY: 10,
                })
            );
            await new Promise((resolve) => setTimeout(resolve, 10));
        });

        const opened = await waitFor(() =>
            Array.from(
                document.querySelectorAll('[data-slot="context-menu-item"]')
            ).some((item) => item.textContent.includes("Merge into current"))
        );
        expect(opened).toBe(true);

        const mergeItem = Array.from(
            document.querySelectorAll('[data-slot="context-menu-item"]')
        ).find((item) => item.textContent.includes("Merge into current"))!;
        await click(mergeItem);

        const settled = await waitFor(() => services.mergeCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.mergeCalls[0]).toBe("develop");
        expect(services.checkoutCalls).toHaveLength(0);
        view.unmount();
    });

    it("keeps the listing cache coherent after switching", async () => {
        const services = listingBackend(listing);
        const view = await openSelector(services);

        const developRow = Array.from(
            document.querySelectorAll('[data-slot="command-item"]')
        ).find((button) => button.textContent.includes("develop"))!;
        await click(developRow);
        await waitFor(() => services.checkoutCalls.length > 0);

        // Checkout drops the stale ahead/behind cache for this repo, which
        // makes the still-mounted selector refetch fresh data.
        await waitFor(() => services.listingFetches() > 1);
        expect(services.listingFetches()).toBeGreaterThan(1);
        view.unmount();
    });
});
