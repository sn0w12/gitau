// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { act, useEffect } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ConfirmProvider } from "@/contexts/confirm-context";
import {
    useRemoveRepository,
    type RemoveRepositoryOutcome,
} from "@/hooks/repositories/use-remove-repository";
import type { RemoveRepositoryResult } from "@/lib/backend/protocol";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import type { Result } from "@/lib/backend/transport/result";
import {
    activateTab,
    appStore,
    createTabRecord,
    openTab,
} from "@/stores/app-store";
import { repositoryStore } from "@/stores/repository-store";
import { seedSettingsForTests } from "@/stores/settings-store";

const mocks = vi.hoisted(() => ({
    removeCalls: [] as Array<{ path: string; moveToTrash: boolean }>,
    removeOutcome: {
        ok: true,
        value: { removedRepoId: 5, repoRoot: "C:\\repos\\alpha" },
    } as Result<RemoveRepositoryResult>,
}));

vi.mock("@/contexts/services-context", () => ({
    useAppServices: () => ({
        backend: {
            repositories: {
                remove: async (path: string, moveToTrash: boolean) => {
                    mocks.removeCalls.push({ path, moveToTrash });
                    return mocks.removeOutcome as Result<RemoveRepositoryResult>;
                },
            },
        },
    }),
}));

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const REPO_PATH = "C:\\repos\\alpha";

const captured: {
    remove: ((path: string) => Promise<RemoveRepositoryOutcome>) | null;
} = { remove: null };

function Harness() {
    const remove = useRemoveRepository();
    useEffect(() => {
        captured.remove = remove;
    }, [remove]);
    return null;
}

function renderHarness() {
    render(
        <ConfirmProvider>
            <Harness />
        </ConfirmProvider>
    );
}

function seedState() {
    const bound = createTabRecord({ title: "alpha", repoPath: REPO_PATH });
    const home = createTabRecord({ title: "Home" });
    openTab(bound);
    openTab(home);
    activateTab(bound.tabId);
    repositoryStore.setState(() => ({
        entries: new Map([[REPO_PATH, { path: REPO_PATH, addedAt: 1 }]]),
    }));
}

function resetStores() {
    repositoryStore.setState(() => ({ entries: new Map() }));
    appStore.setState(() => ({ tabs: [], activeTabId: null }));
}

beforeEach(() => {
    seedSettingsForTests({ closeRepoWithLastTab: true });
    resetStores();
    mocks.removeCalls = [];
    mocks.removeOutcome = {
        ok: true,
        value: { removedRepoId: 5, repoRoot: REPO_PATH },
    };
    document.body.innerHTML = "";
});

afterEach(() => {
    cleanup();
    resetStores();
});

describe("useRemoveRepository", () => {
    it("does nothing when the dialog is cancelled", async () => {
        seedState();
        renderHarness();

        let promise: Promise<RemoveRepositoryOutcome> | undefined;
        await act(async () => {
            promise = captured.remove!(REPO_PATH);
        });
        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
        });
        const outcome = await promise!;

        expect(outcome).toEqual({ status: "cancelled" });
        expect(mocks.removeCalls).toEqual([]);
        expect(repositoryStore.state.entries.get(REPO_PATH)).toBeDefined();
        expect(appStore.state.tabs).toHaveLength(2);
    });

    it("removes from the list only when the checkbox stays unchecked", async () => {
        seedState();
        renderHarness();

        let promise: Promise<RemoveRepositoryOutcome> | undefined;
        await act(async () => {
            promise = captured.remove!(REPO_PATH);
        });
        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Remove" }));
        });
        const outcome = await promise!;

        expect(outcome).toEqual({ status: "removed" });
        expect(mocks.removeCalls).toEqual([
            { path: REPO_PATH, moveToTrash: false },
        ]);
        expect(repositoryStore.state.entries.get(REPO_PATH)).toBeUndefined();
        const boundTab = appStore.state.tabs.find(
            (tab) => tab.repoPath === REPO_PATH
        );
        expect(boundTab).toBeUndefined();
        expect(appStore.state.tabs).toHaveLength(1);
        expect(appStore.state.tabs[0]?.title).toBe("Home");
    });

    it("requests trash when the checkbox is checked", async () => {
        seedState();
        renderHarness();

        let promise: Promise<RemoveRepositoryOutcome> | undefined;
        await act(async () => {
            promise = captured.remove!(REPO_PATH);
        });
        await act(async () => {
            fireEvent.click(screen.getByRole("checkbox"));
        });
        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Remove" }));
        });
        const outcome = await promise!;

        expect(outcome).toEqual({ status: "removed" });
        expect(mocks.removeCalls).toEqual([
            { path: REPO_PATH, moveToTrash: true },
        ]);
        expect(repositoryStore.state.entries.get(REPO_PATH)).toBeUndefined();
    });

    it("keeps the entry and tabs when the backend fails", async () => {
        seedState();
        mocks.removeOutcome = {
            ok: false,
            error: new GitBackendError({
                code: "trashFailed",
                message: "file is in use",
                retryable: false,
            }),
        };
        renderHarness();

        let promise: Promise<RemoveRepositoryOutcome> | undefined;
        await act(async () => {
            promise = captured.remove!(REPO_PATH);
        });
        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Remove" }));
        });
        const outcome = await promise!;

        expect(outcome.status).toBe("failed");
        if (outcome.status === "failed") {
            expect(outcome.error.code).toBe("trashFailed");
        }
        expect(repositoryStore.state.entries.get(REPO_PATH)).toBeDefined();
        expect(appStore.state.tabs).toHaveLength(2);
    });

    it("keeps a surviving tab unbound at home when every tab is bound", async () => {
        const only = createTabRecord({ title: "alpha", repoPath: REPO_PATH });
        openTab(only);
        activateTab(only.tabId);
        repositoryStore.setState(() => ({
            entries: new Map([[REPO_PATH, { path: REPO_PATH, addedAt: 1 }]]),
        }));
        renderHarness();

        let promise: Promise<RemoveRepositoryOutcome> | undefined;
        await act(async () => {
            promise = captured.remove!(REPO_PATH);
        });
        await act(async () => {
            fireEvent.click(screen.getByRole("button", { name: "Remove" }));
        });
        await promise!;

        expect(appStore.state.tabs).toHaveLength(1);
        expect(appStore.state.tabs[0]?.tabId).toBe(only.tabId);
        expect(appStore.state.tabs[0]?.repoPath).toBeUndefined();
    });
});
