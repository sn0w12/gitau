import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { afterEach, describe, expect, it } from "vitest";

import { ChangesPanel } from "@/components/repo/changes/changes-panel";
import { ToastProvider } from "@/components/ui/toast";
import { ConfirmProvider } from "@/contexts/confirm-context";
import { AppServicesContext } from "@/contexts/services-context";
import type {
    RepoSnapshot,
    StashEntry,
    StatusEntry,
    StatusReport,
} from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import type { Result } from "@/lib/backend/transport/result";
import { repositoryStore } from "@/stores/repository-store";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

function entry(
    overrides: Partial<StatusEntry> & { path: string }
): StatusEntry {
    return {
        id: `worktree:${overrides.path}`,
        side: "worktree",
        kind: "modified",
        ...overrides,
    };
}

const report = (entries: StatusEntry[]): Result<StatusReport> => ({
    ok: true,
    value: {
        snapshotId: 1,
        generation: 3,
        entries,
        conflicts: [],
    },
});

const okVoid = (): Promise<Result<void>> =>
    Promise.resolve({ ok: true, value: undefined });

function recordingBackend(
    statusResult: Result<StatusReport>,
    stashes: StashEntry[] = []
) {
    const stageCalls: string[][] = [];
    const unstageCalls: string[][] = [];
    const discardCalls: string[][] = [];
    const editorCalls: Array<{ path: string; relativePath?: string }> = [];
    const revealCalls: Array<{ path: string; relativePath?: string }> = [];
    const stashPushCalls: unknown[] = [];
    const stashPopCalls: Array<{ index: number; action?: string }> = [];
    const backend = {
        changes: {
            status: async (): Promise<Result<StatusReport>> => statusResult,
            stagePaths: (
                _repoId: number,
                paths: string[]
            ): Promise<Result<void>> => {
                stageCalls.push(paths);
                return okVoid();
            },
            unstagePaths: (
                _repoId: number,
                paths: string[]
            ): Promise<Result<void>> => {
                unstageCalls.push(paths);
                return okVoid();
            },
            discardChanges: (
                _repoId: number,
                paths: string[]
            ): Promise<Result<void>> => {
                discardCalls.push(paths);
                return okVoid();
            },
        },
        workflows: {
            stashList: async (): Promise<Result<StashEntry[]>> => ({
                ok: true,
                value: stashes,
            }),
            stashPush: async (
                _repoId: number,
                options: unknown
            ): Promise<Result<string>> => {
                stashPushCalls.push(options);
                return { ok: true, value: "0123456789abcdef" };
            },
            stashPop: async (
                _repoId: number,
                index: number,
                action: string | undefined
            ): Promise<Result<StashEntry[]>> => {
                stashPopCalls.push({ index, action });
                return { ok: true, value: stashes };
            },
        },
        editor: {
            openInEditor: (
                path: string,
                relativePath?: string
            ): Promise<Result<void>> => {
                editorCalls.push({ path, relativePath });
                return okVoid();
            },
        },
        fileManager: {
            reveal: (
                path: string,
                relativePath?: string
            ): Promise<Result<void>> => {
                revealCalls.push({ path, relativePath });
                return okVoid();
            },
        },
    };
    return {
        backend: backend as unknown as BackendClient,
        queryClient: new QueryClient({
            defaultOptions: { queries: { retry: false } },
        }),
        stageCalls,
        unstageCalls,
        discardCalls,
        editorCalls,
        revealCalls,
        stashPushCalls,
        stashPopCalls,
    };
}

type ServicesWithRecording = ReturnType<typeof recordingBackend>;

function renderWith(services: ServicesWithRecording, ui: ReactElement) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(
            <AppServicesContext.Provider value={services}>
                <QueryClientProvider client={services.queryClient}>
                    <ToastProvider>
                        <ConfirmProvider>{ui}</ConfirmProvider>
                    </ToastProvider>
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

async function click(element: Element): Promise<void> {
    await act(async () => {
        element.dispatchEvent(
            new MouseEvent("click", { bubbles: true, cancelable: true })
        );
        await new Promise((resolve) => setTimeout(resolve, 10));
    });
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

async function typeInto(input: HTMLInputElement, value: string): Promise<void> {
    await act(async () => {
        const setter = Object.getOwnPropertyDescriptor(
            HTMLInputElement.prototype,
            "value"
        )!.set!;
        setter.call(input, value);
        input.dispatchEvent(new Event("input", { bubbles: true }));
        await new Promise((resolve) => setTimeout(resolve, 10));
    });
}

function buttonByText(text: string): HTMLButtonElement | undefined {
    return Array.from(document.querySelectorAll("button")).find(
        (button) => button.textContent === text
    );
}

function dialogTitleIncludes(fragment: string): boolean {
    return Array.from(
        document.querySelectorAll('[data-slot="dialog-title"]')
    ).some((heading) => heading.textContent?.includes(fragment));
}

function menuItem(testId: string): Element {
    const item = document.querySelector(`[data-testid="${testId}"]`);
    expect(item).not.toBeNull();
    return item!;
}

/**
 * Base UI portals (menus, dialogs) attach outside the rendered container;
 * poll the document until `predicate` passes or the budget runs out.
 */
async function waitFor(predicate: () => boolean): Promise<boolean> {
    const deadline = Date.now() + 1_000;
    while (Date.now() < deadline) {
        if (predicate()) return true;
        await new Promise((resolve) => setTimeout(resolve, 25));
    }
    return predicate();
}

describe("ChangesPanel", () => {
    it("renders staged and unstaged entries", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(
            report([
                entry({ id: "index:a.ts", side: "index", path: "a.ts" }),
                entry({ path: "b.ts" }),
            ])
        );

        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );

        await flush();
        expect(view.container.textContent).toContain("a.ts");
        expect(view.container.textContent).toContain("b.ts");
        // Section counts render as badges inside each collapsible trigger:
        // one staged, one unstaged.
        const counts = Array.from(
            view.container.querySelectorAll('[data-slot="collapsible-trigger"]')
        ).map(
            (trigger) =>
                trigger.querySelector('[data-slot="badge"]')?.textContent
        );
        expect(counts).toEqual(["1", "1"]);
        view.unmount();
    });

    it("stages a single file from its row button", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(
            report([entry({ path: "src/b.ts" })])
        );

        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );

        await flush();
        const stageButton = view.container.querySelector(
            'button[aria-label="Stage src/b.ts"]'
        );
        expect(stageButton).not.toBeNull();
        await click(stageButton!);

        const settled = await waitFor(() => services.stageCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.stageCalls[0]).toEqual(["src/b.ts"]);
        expect(services.discardCalls).toHaveLength(0);
        view.unmount();
    });

    it("confirms before discarding one file and skips on cancel", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(report([entry({ path: "a.ts" })]));

        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        await click(
            view.container.querySelector('[aria-label="Discard all changes"]')!
        );

        // Confirm opens the destructive dialog first.
        const opened = await waitFor(() =>
            Array.from(
                document.querySelectorAll('[data-slot="dialog-title"]')
            ).some((heading) => heading.textContent.includes("Discard changes"))
        );
        expect(opened).toBe(true);

        const cancelButton = Array.from(
            document.querySelectorAll("button")
        ).find((button) => button.textContent === "Cancel");
        expect(cancelButton).toBeDefined();
        await click(cancelButton!);

        const drained = await waitFor(
            () => !document.querySelector('[data-slot="dialog-popup"]')
        );
        expect(drained).toBe(true);
        expect(services.discardCalls).toHaveLength(0);

        view.unmount();
    });

    it("discards every listed worktree file after confirmation", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(
            report([
                entry({ path: "tracked.ts" }),
                entry({ path: "new-file.ts", kind: "untracked" }),
            ])
        );

        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        await click(
            view.container.querySelector('[aria-label="Discard all changes"]')!
        );
        await waitFor(() =>
            Array.from(
                document.querySelectorAll('[data-slot="dialog-title"]')
            ).some((heading) => heading.textContent.includes("files?"))
        );

        const discardButton = Array.from(
            document.querySelectorAll("button")
        ).find((button) => button.textContent === "Discard");
        expect(discardButton).toBeDefined();
        await click(discardButton!);

        const settled = await waitFor(() => services.discardCalls.length > 0);
        expect(settled).toBe(true);
        // Explicit paths (not the backend `all` flag) so untracked files
        // are removed too.
        expect(services.discardCalls[0]).toEqual(["tracked.ts", "new-file.ts"]);
        view.unmount();
    });

    it("moves a staged file into the staged section after the write", async () => {
        seedSettingsForTests({});
        let staged = false;
        const stageCalls: string[][] = [];
        const backend = {
            changes: {
                status: async (): Promise<Result<StatusReport>> =>
                    report([
                        staged
                            ? entry({
                                  id: "index:a.ts",
                                  side: "index",
                                  path: "a.ts",
                              })
                            : entry({ path: "a.ts" }),
                    ]),
                stagePaths: async (
                    _repoId: number,
                    paths: string[]
                ): Promise<Result<void>> => {
                    stageCalls.push(paths);
                    staged = true;
                    return okVoid();
                },
                unstagePaths: (
                    _repoId: number,
                    _paths: string[]
                ): Promise<Result<void>> => okVoid(),
                discardChanges: (
                    _repoId: number,
                    _paths: string[]
                ): Promise<Result<void>> => okVoid(),
            },
        };
        const services = {
            backend: backend as unknown as BackendClient,
            queryClient: new QueryClient({
                defaultOptions: { queries: { retry: false } },
            }),
            stageCalls: stageCalls as unknown as string[][],
            unstageCalls: [] as string[][],
            discardCalls: [] as string[][],
            editorCalls: [] as Array<{
                path: string;
                relativePath?: string;
            }>,
            revealCalls: [] as Array<{
                path: string;
                relativePath?: string;
            }>,
            stashPushCalls: [] as unknown[],
            stashPopCalls: [] as Array<{ index: number; action?: string }>,
        };

        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );

        await flush();
        expect(view.container.textContent).toContain("a.ts");
        expect(
            view.container.querySelector('button[aria-label="Stage a.ts"]')
        ).not.toBeNull();

        await click(
            view.container.querySelector('button[aria-label="Stage a.ts"]')!
        );
        await waitFor(() => stageCalls.length > 0);

        // The invalidation must refetch status and the refetched report
        // moves the row to the staged section.
        const moved = await waitFor(() => {
            const stagedRow = view.container.querySelector(
                '[aria-label="Unstage a.ts"]'
            );
            return stagedRow !== null;
        });
        expect(moved).toBe(true);
        const counts = Array.from(
            view.container.querySelectorAll('[data-slot="collapsible-trigger"]')
        ).map(
            (trigger) =>
                trigger.querySelector('[data-slot="badge"]')?.textContent
        );
        expect(counts).toEqual(["1", "0"]);
        view.unmount();
    });

    it("retries a stale-snapshot rejection with a refreshed snapshot", async () => {
        seedSettingsForTests({});
        let stageAttempts = 0;
        const stageGenerations: Array<number | undefined> = [];
        const backend = {
            repositories: {
                snapshot: async (): Promise<Result<RepoSnapshot>> =>
                    // First read carries generation 1; the retry's fresh
                    // fetch advances to 2, matching the backend's view.
                    ({
                        ok: true,
                        value: {
                            id: 5,
                            generation: stageAttempts >= 1 ? 2 : 1,
                            workdir: "/tmp/work",
                            gitDir: "/tmp/work/.git",
                            head: {
                                state: "attached",
                                branch: "main",
                                target: "abc123",
                            },
                            shaKind: "sha1",
                        },
                    }),
            },
            changes: {
                status: async (): Promise<Result<StatusReport>> =>
                    report([entry({ path: "a.ts" })]),
                stagePaths: async (
                    _repoId: number,
                    _paths: string[],
                    _all: boolean | undefined,
                    expectedGeneration: number | undefined
                ): Promise<Result<void>> => {
                    stageAttempts++;
                    stageGenerations.push(expectedGeneration);
                    if (stageAttempts === 1) {
                        // The write never ran; the backend rejects it as
                        // stale because the snapshot is behind.
                        return {
                            ok: false,
                            error: new GitBackendError({
                                code: "staleSnapshot",
                                message: "snapshot is stale",
                                retryable: false,
                            }),
                        };
                    }
                    return okVoid();
                },
                unstagePaths: (
                    _repoId: number,
                    _paths: string[]
                ): Promise<Result<void>> => okVoid(),
                discardChanges: (
                    _repoId: number,
                    _paths: string[]
                ): Promise<Result<void>> => okVoid(),
            },
        };
        const services = {
            backend: backend as unknown as BackendClient,
            queryClient: new QueryClient({
                defaultOptions: { queries: { retry: false } },
            }),
        };

        const view = renderWith(
            services as unknown as ServicesWithRecording,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );

        await flush();
        await click(
            view.container.querySelector('button[aria-label="Stage a.ts"]')!
        );

        const settled = await waitFor(() => stageAttempts >= 2);
        expect(settled).toBe(true);
        // First attempt carried no snapshot (not yet cached); the retry
        // refreshed the snapshot and sent its authoritative generation.
        expect(stageGenerations[1]).toBe(2);
        view.unmount();
    });

    it("stages only the checked files when some rows are selected", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(
            report([entry({ path: "one.ts" }), entry({ path: "two.ts" })])
        );

        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        await click(
            view.container.querySelector('[aria-label="Select two.ts"]')!
        );
        await click(
            view.container.querySelector('[aria-label="Stage 1 selected"]')!
        );

        const settled = await waitFor(() => services.stageCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.stageCalls[0]).toEqual(["two.ts"]);
        view.unmount();
    });

    it("header checkbox selects all, turns indeterminate, then clears", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(
            report([entry({ path: "one.ts" }), entry({ path: "two.ts" })])
        );

        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        const header = () =>
            view.container.querySelector('[aria-label="Select all Changes"]')!;
        const rowChecked = (path: string) =>
            view.container
                .querySelector(`[aria-label="Select ${path}"]`)!
                .getAttribute("aria-checked");

        await click(header());
        expect(rowChecked("one.ts")).toBe("true");
        expect(rowChecked("two.ts")).toBe("true");
        expect(
            view.container.querySelector('[aria-label="Stage 2 selected"]')
        ).not.toBeNull();

        await click(
            view.container.querySelector('[aria-label="Select one.ts"]')!
        );
        expect(header().getAttribute("aria-checked")).toBe("mixed");
        expect(
            view.container.querySelector('[aria-label="Stage 1 selected"]')
        ).not.toBeNull();

        // Indeterminate header click selects everything again.
        await click(header());
        expect(rowChecked("one.ts")).toBe("true");
        expect(rowChecked("two.ts")).toBe("true");

        // Fully checked header click clears everything.
        await click(header());
        expect(rowChecked("one.ts")).toBe("false");
        expect(rowChecked("two.ts")).toBe("false");
        expect(
            view.container.querySelector('[aria-label="Stage all changes"]')
        ).not.toBeNull();
        view.unmount();
    });

    it("unstages only the checked staged files", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(
            report([
                entry({
                    id: "index:one.ts",
                    side: "index",
                    path: "one.ts",
                }),
                entry({
                    id: "index:two.ts",
                    side: "index",
                    path: "two.ts",
                }),
            ])
        );

        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        await click(
            view.container.querySelector('[aria-label="Select two.ts"]')!
        );
        await click(
            view.container.querySelector('[aria-label="Unstage 1 selected"]')!
        );

        const settled = await waitFor(() => services.unstageCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.unstageCalls[0]).toEqual(["two.ts"]);
        view.unmount();
    });

    it("opens a file in the configured editor from the row menu", async () => {
        seedSettingsForTests({ editorCommand: "code" });
        repositoryStore.setState(() => ({
            entries: new Map([
                ["/work", { path: "/work", addedAt: 1, repoId: 5 }],
            ]),
        }));
        const services = recordingBackend(
            report([entry({ path: "src/b.ts" })])
        );

        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        const row = view.container.querySelector('[role="option"]');
        expect(row).not.toBeNull();
        await rightClick(row!);
        const opened = await waitFor(() =>
            Boolean(document.querySelector('[data-slot="context-menu-popup"]'))
        );
        expect(opened).toBe(true);

        await click(menuItem("change-menu-open-in-editor"));
        const settled = await waitFor(() => services.editorCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.editorCalls[0]).toEqual({
            path: "/work",
            relativePath: "src/b.ts",
        });
        view.unmount();
    });

    it("reveals a file in the file manager from the row menu", async () => {
        seedSettingsForTests({});
        repositoryStore.setState(() => ({
            entries: new Map([
                ["/work", { path: "/work", addedAt: 1, repoId: 5 }],
            ]),
        }));
        const services = recordingBackend(
            report([entry({ path: "src/b.ts" })])
        );

        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        const row = view.container.querySelector('[role="option"]');
        expect(row).not.toBeNull();
        await rightClick(row!);
        const opened = await waitFor(() =>
            Boolean(document.querySelector('[data-slot="context-menu-popup"]'))
        );
        expect(opened).toBe(true);

        await click(menuItem("change-menu-reveal-in-file-manager"));
        const settled = await waitFor(() => services.revealCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.revealCalls[0]).toEqual({
            path: "/work",
            relativePath: "src/b.ts",
        });
        view.unmount();
    });

    it("stages every unstaged path at once without confirming", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(
            report([
                entry({ path: "one.ts" }),
                entry({ path: "two.ts" }),
                entry({
                    id: "index:three.ts",
                    side: "index",
                    path: "three.ts",
                }),
            ])
        );

        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        await click(
            view.container.querySelector('[aria-label="Stage all changes"]')!
        );

        const settled = await waitFor(() => services.stageCalls.length > 0);
        expect(settled).toBe(true);
        expect(services.stageCalls[0]).toEqual(["one.ts", "two.ts"]);
        view.unmount();
    });

    it("stashes all changes from the header button", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(report([entry({ path: "a.ts" })]));
        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        await click(
            view.container.querySelector('[aria-label="Stash all changes"]')!
        );
        const opened = await waitFor(() =>
            dialogTitleIncludes("Stash all changes")
        );
        expect(opened).toBe(true);

        await typeInto(
            document.querySelector<HTMLInputElement>("#stash-message")!,
            "wip"
        );
        await click(buttonByText("Stash")!);

        const settled = await waitFor(
            () => services.stashPushCalls.length === 1
        );
        expect(settled).toBe(true);
        expect(services.stashPushCalls[0]).toEqual({
            message: "wip",
            includeUntracked: false,
        });
        view.unmount();
    });

    it("stashes only checked changes from the header button", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(
            report([entry({ path: "a.ts" }), entry({ path: "b.ts" })])
        );
        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        await click(
            view.container.querySelector('[aria-label="Select a.ts"]')!
        );
        const button = view.container.querySelector(
            '[aria-label="Stash 1 selected"]'
        );
        expect(button).not.toBeNull();
        await click(button!);

        const opened = await waitFor(() =>
            dialogTitleIncludes("Stash changes")
        );
        expect(opened).toBe(true);
        await click(buttonByText("Stash")!);

        const settled = await waitFor(
            () => services.stashPushCalls.length === 1
        );
        expect(settled).toBe(true);
        expect(services.stashPushCalls[0]).toEqual({
            includeUntracked: false,
            paths: ["a.ts"],
        });
        view.unmount();
    });

    it("stashes the right-clicked path from a row context menu", async () => {
        seedSettingsForTests({});
        repositoryStore.setState(() => ({
            entries: new Map([
                ["/work", { path: "/work", addedAt: 1, repoId: 5 }],
            ]),
        }));
        const services = recordingBackend(report([entry({ path: "a.ts" })]));
        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        const row = view.container.querySelector('[role="option"]');
        expect(row).not.toBeNull();
        await rightClick(row!);
        await waitFor(() =>
            Boolean(document.querySelector('[data-slot="context-menu-popup"]'))
        );
        await click(menuItem("change-menu-stash"));

        const opened = await waitFor(() =>
            dialogTitleIncludes("Stash changes")
        );
        expect(opened).toBe(true);
        expect(services.stashPushCalls).toHaveLength(0);

        await click(buttonByText("Stash")!);
        const settled = await waitFor(
            () => services.stashPushCalls.length === 1
        );
        expect(settled).toBe(true);
        expect(services.stashPushCalls[0]).toEqual({
            includeUntracked: false,
            paths: ["a.ts"],
        });
        view.unmount();
    });

    it("restores a stash from the stashed changes list", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(report([entry({ path: "a.ts" })]), [
            { index: 0, message: "WIP on main: fix", commit: "abcdef012345" },
        ]);
        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        const hasTrigger = await waitFor(
            () =>
                view.container.querySelector(
                    '[data-testid="stashed-changes-trigger"]'
                ) !== null
        );
        expect(hasTrigger).toBe(true);
        await click(
            view.container.querySelector(
                '[data-testid="stashed-changes-trigger"]'
            )!
        );

        const listed = await waitFor(() =>
            view.container.textContent!.includes("WIP on main: fix")
        );
        expect(listed).toBe(true);
        expect(view.container.textContent).toContain("Stashed Changes");

        await click(
            view.container.querySelector('[aria-label="Restore stash 0"]')!
        );
        const settled = await waitFor(
            () => services.stashPopCalls.length === 1
        );
        expect(settled).toBe(true);
        expect(services.stashPopCalls[0]).toEqual({ index: 0, action: "pop" });
        view.unmount();
    });

    it("confirms before dropping a stash", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(report([entry({ path: "a.ts" })]), [
            { index: 0, message: "WIP on main: fix", commit: "abcdef012345" },
        ]);
        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        const hasTrigger = await waitFor(
            () =>
                view.container.querySelector(
                    '[data-testid="stashed-changes-trigger"]'
                ) !== null
        );
        expect(hasTrigger).toBe(true);
        await click(
            view.container.querySelector(
                '[data-testid="stashed-changes-trigger"]'
            )!
        );

        const dropReady = await waitFor(() => {
            const button = view.container.querySelector(
                '[aria-label="Drop stash 0"]'
            );
            return button !== null;
        });
        expect(dropReady).toBe(true);

        await click(
            view.container.querySelector('[aria-label="Drop stash 0"]')!
        );
        const opened = await waitFor(() =>
            dialogTitleIncludes("Drop stash@{0}?")
        );
        expect(opened).toBe(true);
        expect(services.stashPopCalls).toHaveLength(0);

        await click(buttonByText("Drop")!);
        const settled = await waitFor(
            () => services.stashPopCalls.length === 1
        );
        expect(settled).toBe(true);
        expect(services.stashPopCalls[0]).toEqual({ index: 0, action: "drop" });
        view.unmount();
    });

    it("selects a stash so the main view can show it", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(report([entry({ path: "a.ts" })]), [
            { index: 0, message: "WIP on main: fix", commit: "abcdef012345" },
        ]);
        const selected: Array<string | null> = [];
        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={() => {}}
                selectedStashId={null}
                onSelectedStash={(id) => selected.push(id)}
            />
        );
        await flush();

        const hasTrigger = await waitFor(
            () =>
                view.container.querySelector(
                    '[data-testid="stashed-changes-trigger"]'
                ) !== null
        );
        expect(hasTrigger).toBe(true);
        await click(
            view.container.querySelector(
                '[data-testid="stashed-changes-trigger"]'
            )!
        );

        const row = view.container.querySelector(
            '[data-testid="stashed-changes"] [role="option"]'
        );
        expect(row).not.toBeNull();
        await click(row!);
        expect(selected).toContain("abcdef012345");
        view.unmount();
    });

    it("opens conflicted files in the 3-way viewer id", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(
            report([
                entry({
                    id: "worktree:a.ts",
                    side: "worktree",
                    path: "a.ts",
                    kind: "conflicted",
                }),
            ])
        );
        const selected: Array<string | null> = [];
        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={(id) => selected.push(id)}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        const row = view.container.querySelector('[role="option"]');
        expect(row?.textContent).toContain("a.ts");
        await click(row!);
        expect(selected).toEqual(["conflict:a.ts"]);
        view.unmount();
    });

    it("keeps plain selection ids for non-conflicted files", async () => {
        seedSettingsForTests({});
        const services = recordingBackend(report([entry({ path: "b.ts" })]));
        const selected: Array<string | null> = [];
        const view = renderWith(
            services,
            <ChangesPanel
                repoId={5}
                branch="main"
                selectedId={null}
                onSelectedChange={(id) => selected.push(id)}
                selectedStashId={null}
                onSelectedStash={() => {}}
            />
        );
        await flush();

        await click(view.container.querySelector('[role="option"]')!);
        expect(selected).toEqual(["worktree:b.ts"]);
        view.unmount();
    });
});

afterEach(() => {
    repositoryStore.setState(() => ({ entries: new Map() }));
});

async function flush(): Promise<void> {
    await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 20));
    });
}
