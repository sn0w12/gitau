import { RouterProvider } from "@tanstack/react-router";
import { cleanup, render } from "@testing-library/react";
// @vitest-environment jsdom
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";

import { createAppServices } from "@/lib/bootstrap/app-runtime";
import type { AppServices } from "@/lib/bootstrap/app-runtime";
import { createTabRouter } from "@/lib/routing/tab-router-factory";
import { appStore, createTabRecord, openTab } from "@/stores/app-store";
import { registerOpen } from "@/stores/repository-store";
import {
    resetRuntimesForTests,
    getOrCreateRuntime,
} from "@/stores/tab-runtime";

// The route tree imports the home page, which pulls in @dnd-kit; its dom
// package instantiates a ResizeObserver-based ResizeNotifier at import time.
const globalsInstalled = vi.hoisted(() => {
    const globalScope = globalThis as Record<string, unknown>;

    globalScope.ResizeObserver ??= class {
        observe(): void {}
        unobserve(): void {}
        disconnect(): void {}
    };

    globalScope.matchMedia ??= () =>
        ({
            matches: false,
            addEventListener: (): void => {},
            removeEventListener: (): void => {},
            addListener: (): void => {},
            removeListener: (): void => {},
            onchange: null,
            dispatchEvent: (): boolean => false,
        }) as unknown as MediaQueryList;
});
void globalsInstalled;

function makeTab(title: string) {
    const record = createTabRecord({ title });
    openTab(record);
    const runtime = getOrCreateRuntime(record);
    return { record, runtime };
}

describe("per-tab routers", () => {
    let services: AppServices;

    beforeEach(() => {
        resetRuntimesForTests();
        services = createAppServices();
    });

    afterEach(() => {
        cleanup();
    });

    it("keeps locations isolated between tabs", async () => {
        const a = makeTab("a");
        const b = makeTab("b");
        const routerA = createTabRouter(a.record.tabId, "/", services);
        const routerB = createTabRouter(b.record.tabId, "/", services);

        await routerA.navigate({ to: "/", hash: "only-a" });

        expect(routerA.state.location.hash).toBe("only-a");
        expect(routerB.state.location.hash).toBe("");
    });

    it("keeps back/forward history isolated between tabs", async () => {
        const a = makeTab("a");
        const b = makeTab("b");
        const routerA = createTabRouter(a.record.tabId, "/", services);
        const routerB = createTabRouter(b.record.tabId, "/", services);

        await routerA.navigate({ to: "/", hash: "first" });
        await routerA.navigate({ to: "/", hash: "second" });

        // History listeners attach on mount; render so back() is processed.
        render(<RouterProvider router={routerA} />);

        const resolved = new Promise<void>((resolve) => {
            const unsubscribe = routerA.subscribe("onResolved", () => {
                unsubscribe();
                resolve();
            });
        });
        routerA.history.back();
        await resolved;

        expect(routerA.state.location.hash).toBe("first");
        expect(routerB.state.location.hash).toBe("");
    });

    it("mirrors resolved hrefs into the owning tab record", async () => {
        const tab = makeTab("mirror");
        const router = createTabRouter(tab.record.tabId, "/", services);

        await router.navigate({ to: "/", hash: "resolved" });

        const stored = appStore.state.tabs.find(
            (candidate) => candidate.tabId === tab.record.tabId
        );
        expect(stored?.lastResolvedHref).toContain("resolved");
    });

    it("reuses the same router instance for the same tab id", () => {
        const tab = makeTab("reuse");
        const runtime = getOrCreateRuntime(tab.record);
        const router = createTabRouter(tab.record.tabId, "/", services);
        const again = getOrCreateRuntime(tab.record);

        expect(again).toBe(runtime);
        expect(runtime.router).toBe(router);
    });

    it("gives every tab its own route-tree instances", () => {
        const a = makeTab("tree-a");
        const b = makeTab("tree-b");
        const routerA = createTabRouter(a.record.tabId, "/", services);
        const routerB = createTabRouter(b.record.tabId, "/", services);

        expect(routerA.routeTree).not.toBe(routerB.routeTree);
        expect(routerA.routesById["/"]).not.toBe(routerB.routesById["/"]);
    });

    it("binds the tab to the repository an issue route names", () => {
        const tab = makeTab("issue");
        registerOpen("C:/repos/gitau", 5);
        const router = createTabRouter(tab.record.tabId, "/", services);

        const loader = router.routesById["/repo/$repoId/issue/$issueId"].options
            .loader as (context: { params: Record<string, string> }) => void;
        loader({ params: { repoId: "5", issueId: "42" } });

        const stored = appStore.state.tabs.find(
            (candidate) => candidate.tabId === tab.record.tabId
        );
        expect(stored?.repoPath).toBe("C:/repos/gitau");
    });

    it("keeps a replace from growing the tab's history", async () => {
        const tab = makeTab("replace");
        const router = createTabRouter(tab.record.tabId, "/repo/5", services);
        const start = router.history.length;

        await router.navigate({
            to: "/repo/$repoId",
            params: { repoId: "5" },
            search: { view: "graph" },
            replace: true,
        });
        await router.navigate({
            to: "/repo/$repoId",
            params: { repoId: "5" },
            search: { view: "graph", commit: "abc" },
            replace: true,
        });

        expect(router.history.length).toBe(start);
        expect(router.state.location.search).toEqual({
            view: "graph",
            commit: "abc",
        });
    });

    it("validates the repo page query string down to the known keys", () => {
        const tab = makeTab("validate");
        const router = createTabRouter(tab.record.tabId, "/", services);

        const validateSearch = router.routesById["/repo/$repoId"].options
            .validateSearch as (search: Record<string, unknown>) => unknown;

        expect(
            validateSearch({ view: "pulls", tab: "history", junk: "1" })
        ).toEqual({ view: "pulls", tab: "history" });
        expect(validateSearch({ view: "nope" })).toEqual({});
    });
});
