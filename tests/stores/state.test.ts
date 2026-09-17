import { describe, expect, it, beforeEach } from "vitest";

import { RepoTitleBadge } from "@/components/repo/repo-title-badge";
import {
    appStore,
    createTabRecord,
    openTab,
    closeTab,
    activateTab,
    duplicateTab,
    setLastResolvedHref,
    setTabTitle,
    selectActiveTab,
    selectTabsByRepoPath,
} from "@/stores/app-store";
import {
    beginOperation,
    finishOperation,
    forgetOperationsForTab,
    operationStore,
} from "@/stores/operation-store";
import {
    registerOpen,
    ensureRepo,
    unbindRepo,
    repositoryStore,
} from "@/stores/repository-store";
import {
    resetRuntimesForTests,
    getOrCreateRuntime,
    getRuntime,
    disposeRuntime,
} from "@/stores/tab-runtime";

describe("app store", () => {
    beforeEach(() => {
        resetRuntimesForTests();
    });

    it("opens tabs and activates them", () => {
        const a = createTabRecord({ title: "Home" });
        openTab(a);
        expect(appStore.state.activeTabId).toBe(a.tabId);

        const b = createTabRecord({ title: "Repo" });
        openTab(b);
        expect(appStore.state.activeTabId).toBe(b.tabId);
        expect(appStore.state.tabs).toHaveLength(2);
    });

    it("does not duplicate a tab id", () => {
        const tab = createTabRecord({ title: "x" });
        openTab(tab);
        openTab(tab);
        expect(appStore.state.tabs).toHaveLength(1);
    });

    it("activates a neighbor when the active tab closes", () => {
        const a = createTabRecord({ title: "a" });
        openTab(a);
        const b = createTabRecord({ title: "b" });
        openTab(b);
        activateTab(a.tabId);

        closeTab(a.tabId);
        expect(appStore.state.activeTabId).toBe(b.tabId);

        closeTab(b.tabId);
        expect(appStore.state.activeTabId).toBe(b.tabId);
        expect(appStore.state.tabs).toHaveLength(1);
    });

    it("never closes the last tab", () => {
        const only = createTabRecord({ title: "only" });
        openTab(only);

        closeTab(only.tabId);

        expect(appStore.state.tabs).toHaveLength(1);
        expect(appStore.state.tabs[0].tabId).toBe(only.tabId);
        expect(appStore.state.activeTabId).toBe(only.tabId);
    });

    it("mirrors resolved hrefs per tab", () => {
        const a = createTabRecord({ title: "a" });
        const b = createTabRecord({ title: "b" });
        openTab(a);
        openTab(b);

        setLastResolvedHref(a.tabId, "/history");
        setLastResolvedHref(b.tabId, "/status");

        expect(selectActiveTab()?.lastResolvedHref).toBe("/status");
        activateTab(a.tabId);
        expect(selectActiveTab()?.lastResolvedHref).toBe("/history");
    });

    it("filters tabs by repo path", () => {
        const t = createTabRecord({
            title: "r",
            repoPath: "/repo",
        });
        openTab(t);
        expect(selectTabsByRepoPath("/repo")).toHaveLength(1);
        expect(selectTabsByRepoPath("/other")).toHaveLength(0);
    });

    it("duplicates a tab beside itself with copied binding", () => {
        const source = createTabRecord({
            title: "repo",
            repoPath: "/repo",
            initialHref: "/tree/main",
        });
        openTab(source);
        setLastResolvedHref(source.tabId, "/commits");

        const clone = duplicateTab(source);

        expect(clone.tabId).not.toBe(source.tabId);
        expect(appStore.state.tabs.map((tab) => tab.tabId)).toEqual([
            source.tabId,
            clone.tabId,
        ]);
        expect(appStore.state.activeTabId).toBe(clone.tabId);
        expect(clone.repoPath).toBe("/repo");
        expect(clone.lastResolvedHref).toBe("/commits");
    });

    it("sets titles with a badge and resets it when omitted", () => {
        const tab = createTabRecord({ title: "start" });
        openTab(tab);

        const badge = { key: "repo", component: RepoTitleBadge };
        setTabTitle(tab.tabId, "Repo", badge);
        expect(appStore.state.tabs[0].title).toBe("Repo");
        expect(appStore.state.tabs[0].titleBadge).toEqual(badge);

        // Title without a badge clears the previous one.
        setTabTitle(tab.tabId, "Plain");
        expect(appStore.state.tabs[0].title).toBe("Plain");
        expect(appStore.state.tabs[0].titleBadge).toBeUndefined();

        // Unknown tab ids are ignored.
        expect(() => setTabTitle("missing", "x")).not.toThrow();
    });

    it("keeps the state reference when the title is unchanged", () => {
        const tab = createTabRecord({ title: "same" });
        openTab(tab);

        const before = appStore.state;
        setTabTitle(tab.tabId, "same");
        expect(appStore.state).toBe(before);
    });
});

describe("tab runtime registry", () => {
    beforeEach(() => {
        resetRuntimesForTests();
    });

    it("reuses one runtime per tab id", async () => {
        const tab = createTabRecord({ title: "t" });
        openTab(tab);

        const first = getOrCreateRuntime(tab);
        const second = getOrCreateRuntime({ ...tab });
        expect(first).toBe(second);
        expect(getRuntime(tab.tabId)).toBeDefined();

        await disposeRuntime(tab.tabId);
        expect(getRuntime(tab.tabId)).toBeUndefined();
    });

    it("flags disposed runtimes once", async () => {
        const tab = createTabRecord({ title: "t" });
        openTab(tab);
        const runtime = getOrCreateRuntime(tab);
        await disposeRuntime(tab.tabId);
        expect(runtime.disposed).toBe(true);
    });
});

describe("repository store", () => {
    beforeEach(() => {
        repositoryStore.setState(() => ({ entries: new Map() }));
    });

    it("keeps the entry listed when closed", () => {
        registerOpen("/nine", 9);
        unbindRepo(9);
        const entry = [...repositoryStore.state.entries.values()].find(
            (e) => e.path === "/nine"
        );
        // The repo stays known and clickable; only the process-local
        // binding is dropped (the backend reaps the idle session).
        expect(entry).toBeDefined();
        expect(entry?.repoId).toBeUndefined();
    });

    it("dedupes paths case-insensitively on ensure", () => {
        ensureRepo("/Repo");
        ensureRepo("/repo");
        expect(repositoryStore.state.entries.size).toBe(1);
        expect([...repositoryStore.state.entries.keys()][0]).toBe("/Repo");
    });
});

describe("operation store", () => {
    beforeEach(() => {
        operationStore.setState(() => ({ operations: new Map() }));
    });

    it("transitions to exactly one terminal phase", () => {
        beginOperation({ operationId: 1, kind: "diff", tabId: "t1" });
        finishOperation(1, "completed");

        const record = operationStore.state.operations.get(1);
        expect(record?.phase).toBe("completed");
        expect(record?.endedAt).toBeDefined();

        finishOperation(1, "failed");
        expect(operationStore.state.operations.get(1)?.phase).toBe("completed");
    });

    it("forgets operations owned by a closing tab and returns their ids", () => {
        beginOperation({ operationId: 10, kind: "diff", tabId: "t1" });
        beginOperation({ operationId: 11, kind: "fetch", tabId: "t2" });
        beginOperation({ operationId: 12, kind: "diff", tabId: "t1" });

        const ids = forgetOperationsForTab("t1");
        expect(ids.sort()).toEqual([10, 12]);
        expect(operationStore.state.operations.get(11)).toBeDefined();
        expect(operationStore.state.operations.get(10)).toBeUndefined();
    });
});
