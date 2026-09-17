// @vitest-environment jsdom
import { QueryClientProvider, QueryClient } from "@tanstack/react-query";
import { fireEvent, render, screen } from "@testing-library/react";
import { act, useEffect } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { toastManager } from "@/components/ui/toast";
import { ConfirmProvider } from "@/contexts/confirm-context";
import {
    useCommitActions,
    type TagFormInput,
} from "@/hooks/repositories/use-commit-actions";
import type {
    CommitSummary,
    TagInfo,
    WorkflowOutcome,
} from "@/lib/backend/protocol";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import type { Result } from "@/lib/backend/transport/result";
import { seedSettingsForTests } from "@/stores/settings-store";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
    resetCalls: [] as Array<{ repoId: number; kind: string; target: string }>,
    checkoutCalls: [] as string[],
    revertCalls: [] as Array<{ target: string; parentIndex?: number }>,
    cherryPickCalls: [] as string[],
    createTagCalls: [] as Array<{
        name: string;
        target?: string;
        message?: string;
    }>,
    createBranchCalls: [] as Array<{ name: string; startPoint?: string }>,
    revertOutcome: {
        ok: true,
        value: { outcome: "finished", commit: "abc123" },
    } as Result<WorkflowOutcome>,
    createTagOutcome: {
        ok: true,
        value: { name: "v1.0.0", target: "" },
    } as Result<TagInfo>,
}));

vi.mock("@/contexts/services-context", () => ({
    useAppServices: () => ({
        backend: {
            mutations: {
                reset: async (repoId: number, kind: string, target: string) => {
                    mocks.resetCalls.push({ repoId, kind, target });
                    return { ok: true, value: undefined } as Result<void>;
                },
                checkout: async (repoId: number, target: string) => {
                    mocks.checkoutCalls.push(`${repoId}:${target}`);
                    return { ok: true, value: undefined } as Result<void>;
                },
            },
            workflows: {
                revert: async (
                    _repoId: number,
                    target: string,
                    parentIndex?: number
                ) => {
                    mocks.revertCalls.push({ target, parentIndex });
                    return mocks.revertOutcome as Result<WorkflowOutcome>;
                },
                cherryPick: async (_repoId: number, target: string) => {
                    mocks.cherryPickCalls.push(target);
                    return {
                        ok: true,
                        value: { outcome: "finished", commit: "def456" },
                    } as Result<WorkflowOutcome>;
                },
            },
            refs: {
                createTag: async (
                    _repoId: number,
                    name: string,
                    target?: string,
                    message?: string
                ) => {
                    mocks.createTagCalls.push({ name, target, message });
                    return mocks.createTagOutcome as Result<TagInfo>;
                },
                createBranch: async (
                    _repoId: number,
                    name: string,
                    startPoint?: string
                ) => {
                    mocks.createBranchCalls.push({ name, startPoint });
                    return {
                        ok: true,
                        value: {
                            name,
                            target: startPoint ?? "",
                            isHead: false,
                        },
                    };
                },
            },
        },
        queryClient: {
            getQueryData: () => undefined,
            invalidateQueries: async () => {},
        },
    }),
}));

const captured: {
    actions: ReturnType<typeof useCommitActions> | null;
} = { actions: null };

function Harness() {
    const actions = useCommitActions(5);
    useEffect(() => {
        captured.actions = actions;
    }, [actions]);
    return null;
}

function renderHarness() {
    render(
        <QueryClientProvider
            client={
                new QueryClient({
                    defaultOptions: { mutations: { retry: false } },
                })
            }
        >
            <ConfirmProvider>
                <Harness />
            </ConfirmProvider>
        </QueryClientProvider>
    );
}

function commit(overrides: Partial<CommitSummary> = {}): CommitSummary {
    return {
        id: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b",
        treeId: "tree",
        parentIds: [],
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

async function confirmInDialog(label: string): Promise<void> {
    const opened = await vi.waitFor(() =>
        Boolean(document.querySelector('[data-slot="dialog-popup"]'))
    );
    expect(opened).toBe(true);
    const button = screen.getByRole("button", { name: label });
    await act(async () => {
        fireEvent.click(button);
    });
}

beforeEach(() => {
    seedSettingsForTests({});
    mocks.resetCalls = [];
    mocks.checkoutCalls = [];
    mocks.revertCalls = [];
    mocks.cherryPickCalls = [];
    mocks.createTagCalls = [];
    mocks.createBranchCalls = [];
    mocks.revertOutcome = {
        ok: true,
        value: { outcome: "finished", commit: "abc123" },
    };
    mocks.createTagOutcome = {
        ok: true,
        value: { name: "v1.0.0", target: "" },
    };
    document.body.innerHTML = "";
});

afterEach(() => {
    document.body.innerHTML = "";
    vi.restoreAllMocks();
});

describe("useCommitActions", () => {
    it("resets hard to the commit after confirmation", async () => {
        renderHarness();
        const rowCommit = commit();

        await act(async () => {
            captured.actions!.resetTo(rowCommit, "hard");
        });
        await confirmInDialog("Reset");

        await vi.waitFor(() => expect(mocks.resetCalls).toHaveLength(1));
        expect(mocks.resetCalls[0]).toEqual({
            repoId: 5,
            kind: "hard",
            target: rowCommit.id,
        });
    });

    it("skips the reset when cancelled", async () => {
        renderHarness();
        const rowCommit = commit();

        let promise: Promise<void> | undefined;
        await act(async () => {
            promise = captured.actions!.resetTo(rowCommit, "hard");
        });
        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
        });
        await promise;

        expect(mocks.resetCalls).toHaveLength(0);
    });

    it("undoes a commit as a soft reset to its parent", async () => {
        renderHarness();
        const parent = "4444444444444444444444444444444444444444";
        const rowCommit = commit({ parentIds: [parent] });

        await act(async () => {
            captured.actions!.undoCommit(rowCommit);
        });
        await confirmInDialog("Undo commit");

        await vi.waitFor(() => expect(mocks.resetCalls).toHaveLength(1));
        expect(mocks.resetCalls[0]).toEqual({
            repoId: 5,
            kind: "soft",
            target: parent,
        });
    });

    it("does nothing when undoing a root commit", async () => {
        renderHarness();

        await act(async () => {
            await captured.actions!.undoCommit(commit());
        });

        expect(mocks.resetCalls).toHaveLength(0);
        expect(document.querySelector('[data-slot="dialog-popup"]')).toBeNull();
    });

    it("checks out the commit after confirmation", async () => {
        renderHarness();
        const rowCommit = commit();

        await act(async () => {
            captured.actions!.checkoutCommit(rowCommit);
        });
        await confirmInDialog("Checkout");

        await vi.waitFor(() => expect(mocks.checkoutCalls).toHaveLength(1));
        expect(mocks.checkoutCalls[0]).toBe(`5:${rowCommit.id}`);
    });

    it("reverts a merge commit against its first parent and toasts success", async () => {
        renderHarness();
        const toastSpy = vi.spyOn(toastManager, "add");
        const mergeCommit = commit({
            parentIds: [
                "1111111111111111111111111111111111111111",
                "2222222222222222222222222222222222222222",
            ],
        });

        await act(async () => {
            await captured.actions!.revertCommit(mergeCommit);
        });

        expect(mocks.revertCalls).toEqual([
            { target: mergeCommit.id, parentIndex: 1 },
        ]);
        expect(toastSpy).toHaveBeenCalledWith(
            expect.objectContaining({ title: "Revert", type: "success" })
        );
    });

    it("reverts a normal commit without a mainline parent", async () => {
        renderHarness();
        const rowCommit = commit({
            parentIds: ["4444444444444444444444444444444444444444"],
        });

        await act(async () => {
            await captured.actions!.revertCommit(rowCommit);
        });

        expect(mocks.revertCalls).toEqual([
            { target: rowCommit.id, parentIndex: undefined },
        ]);
    });

    it("warns with the conflict paths when a revert stops on conflicts", async () => {
        renderHarness();
        const toastSpy = vi.spyOn(toastManager, "add");
        mocks.revertOutcome = {
            ok: true,
            value: { outcome: "conflicted", paths: ["a.ts", "b.ts", "c.ts"] },
        };

        await act(async () => {
            await captured.actions!.revertCommit(commit());
        });

        expect(toastSpy).toHaveBeenCalledWith(
            expect.objectContaining({
                title: "Revert stopped on conflicts",
                type: "warning",
            })
        );
        const call = toastSpy.mock.calls[0][0];
        expect(call.description).toContain("a.ts");
    });

    it("toasts an error when the revert fails", async () => {
        renderHarness();
        const toastSpy = vi.spyOn(toastManager, "add");
        mocks.revertOutcome = {
            ok: false,
            error: new GitBackendError({
                code: "conflict",
                message: "unresolved conflicts",
                retryable: false,
            }),
        };

        await act(async () => {
            await captured.actions!.revertCommit(commit());
        });

        expect(toastSpy).toHaveBeenCalledWith(
            expect.objectContaining({ type: "error" })
        );
    });

    it("cherry-picks the commit onto HEAD", async () => {
        renderHarness();

        await act(async () => {
            await captured.actions!.cherryPickCommit(commit());
        });

        expect(mocks.cherryPickCalls).toEqual([commit().id]);
    });

    it("creates a tag pointing at the commit", async () => {
        renderHarness();
        const rowCommit = commit();
        const input: TagFormInput = { name: "v1.0.0", message: "release" };

        let ok: boolean | undefined;
        await act(async () => {
            ok = await captured.actions!.createTagHere(rowCommit, input);
        });

        expect(ok).toBe(true);
        expect(mocks.createTagCalls).toEqual([
            { name: "v1.0.0", target: rowCommit.id, message: "release" },
        ]);
    });

    it("creates a branch at the commit without checkout", async () => {
        renderHarness();
        const rowCommit = commit();

        let ok: boolean | undefined;
        await act(async () => {
            ok = await captured.actions!.createBranchHere(rowCommit, "feature");
        });

        expect(ok).toBe(true);
        expect(mocks.createBranchCalls).toEqual([
            { name: "feature", startPoint: rowCommit.id },
        ]);
    });

    it("reports failure when tag creation fails", async () => {
        renderHarness();
        const toastSpy = vi.spyOn(toastManager, "add");
        mocks.createTagOutcome = {
            ok: false,
            error: new GitBackendError({
                code: "invalidInput",
                message: "tag already exists",
                retryable: false,
            }),
        };

        let ok: boolean | undefined;
        await act(async () => {
            ok = await captured.actions!.createTagHere(commit(), {
                name: "v1.0.0",
            });
        });

        expect(ok).toBe(false);
        expect(mocks.createTagCalls).toHaveLength(1);
        expect(toastSpy).toHaveBeenCalledWith(
            expect.objectContaining({
                title: "Could not create tag",
                type: "error",
            })
        );
    });
});
