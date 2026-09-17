import { beforeEach, describe, expect, it, vi } from "vitest";

import {
    closeOtherTabsFully,
    closeTabsToRightFully,
} from "@/lib/routing/tab-lifecycle";
import {
    activateTab,
    appStore,
    createTabRecord,
    openTab,
} from "@/stores/app-store";
import { registerOpen } from "@/stores/repository-store";
import { seedSettingsForTests } from "@/stores/settings-store";
import {
    getOrCreateRuntime,
    getRuntime,
    resetRuntimesForTests,
} from "@/stores/tab-runtime";

function stubBackend() {
    return {
        diff: { cancel: vi.fn(async () => {}) },
        repositories: { close: vi.fn(async () => {}) },
    };
}

function openThree() {
    const a = createTabRecord({ title: "a", repoPath: "/one" });
    const b = createTabRecord({ title: "b" });
    const c = createTabRecord({ title: "c", repoPath: "/two" });
    // Real apps create a runtime once a tab becomes visible.
    for (const tab of [a, b, c]) {
        openTab(tab);
        getOrCreateRuntime(tab);
    }
    return { a, b, c };
}

describe("batch tab closing", () => {
    beforeEach(() => {
        resetRuntimesForTests();
        // Tab close behavior reads settings; bootstrap guarantees they are
        // initialized in the real app.
        seedSettingsForTests({ closeRepoWithLastTab: true });
    });

    it("close others keeps only the anchor tab and activates it", async () => {
        const { b } = openThree();
        activateTab(b.tabId);

        await closeOtherTabsFully(b.tabId);

        expect(appStore.state.tabs.map((tab) => tab.title)).toEqual(["b"]);
        expect(appStore.state.activeTabId).toBe(b.tabId);
        expect(getRuntime("a")).toBeUndefined();
        expect(getRuntime("c")).toBeUndefined();
    });

    it("close others disposes runtimes of removed tabs", async () => {
        const { a, b, c } = openThree();
        const runtimeA = getRuntime(a.tabId);
        const runtimeC = getRuntime(c.tabId);

        await closeOtherTabsFully(b.tabId);

        expect(runtimeA?.disposed).toBe(true);
        expect(runtimeC?.disposed).toBe(true);
    });

    it("close others closes repos whose last reference vanished", async () => {
        const backend = stubBackend();
        const { b } = openThree();
        // Bindings live in the repository store; tabs only carry paths.
        registerOpen("/one", 1);
        registerOpen("/two", 2);

        await closeOtherTabsFully(b.tabId, backend as never);

        expect(backend.repositories.close).toHaveBeenCalledTimes(2);
        expect(backend.repositories.close).toHaveBeenCalledWith(1);
        expect(backend.repositories.close).toHaveBeenCalledWith(2);
    });

    it("closing to the right keeps the anchor and everything left", async () => {
        const { a } = openThree();
        activateTab(a.tabId);

        await closeTabsToRightFully(a.tabId);

        expect(appStore.state.tabs.map((tab) => tab.title)).toEqual(["a"]);
        expect(appStore.state.activeTabId).toBe(a.tabId);
    });

    it("closing to the right of the last tab is a no-op", async () => {
        const { c } = openThree();

        await closeTabsToRightFully(c.tabId);

        expect(appStore.state.tabs).toHaveLength(3);
    });
});
