// @vitest-environment jsdom
import {
    createMemoryHistory,
    createRootRoute,
    createRoute,
    createRouter,
    RouterProvider,
} from "@tanstack/react-router";
import { cleanup, render } from "@testing-library/react";
import { act, useEffect } from "react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { useRepoPageState } from "@/hooks/repositories/use-repo-page-state";
import { validateRepoSearch } from "@/lib/routing/repo-search";
import type { RepoStateAction } from "@/lib/routing/repo-search";
import {
    activateTab,
    appStore,
    createTabRecord,
    openTab,
} from "@/stores/app-store";
import {
    attachRouter,
    getOrCreateRuntime,
    resetRuntimesForTests,
} from "@/stores/tab-runtime";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

const captured: {
    view: string | null;
    selectedCommit: string | null;
    issuesTab: string | null;
    dispatch: ((action: RepoStateAction) => void) | null;
} = { view: null, selectedCommit: null, issuesTab: null, dispatch: null };

function TestPage() {
    const [state, dispatch] = useRepoPageState(5);
    useEffect(() => {
        captured.view = state.view;
        captured.selectedCommit = state.selectedCommit;
        captured.issuesTab = state.issuesTab;
        captured.dispatch = dispatch;
    }, [dispatch, state]);
    return null;
}

const flush = () =>
    act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 0));
    });

// The real tree binds the repo route to the page component, which needs a
// backend. This tree keeps the real validateSearch and swaps the component.
async function mountRepoPage(initialEntries: string[]) {
    const rootRoute = createRootRoute();
    const repoRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/repo/$repoId",
        component: TestPage,
        validateSearch: validateRepoSearch,
    });
    const router = createRouter({
        routeTree: rootRoute.addChildren([repoRoute]),
        history: createMemoryHistory({ initialEntries }),
    });

    const tab = createTabRecord({ title: "repo", repoPath: "/repos/alpha" });
    openTab(tab);
    activateTab(tab.tabId);
    getOrCreateRuntime(tab);
    attachRouter(tab.tabId, router as never);
    render(<RouterProvider router={router} />);
    await flush();
    return router;
}

async function dispatch(action: RepoStateAction) {
    await act(async () => {
        captured.dispatch?.(action);
    });
    await flush();
}

beforeEach(() => {
    resetRuntimesForTests();
    appStore.setState(() => ({ tabs: [], activeTabId: null }));
    captured.view = null;
    captured.selectedCommit = null;
    captured.issuesTab = null;
    captured.dispatch = null;
});

afterEach(() => {
    cleanup();
    resetRuntimesForTests();
    appStore.setState(() => ({ tabs: [], activeTabId: null }));
});

describe("useRepoPageState", () => {
    it("writes each dispatch into the href", async () => {
        const router = await mountRepoPage(["/repo/5?view=issues"]);

        await dispatch({ type: "SET_CHANGE", data: "index:a.ts" });
        expect(router.state.location.search).toEqual({
            view: "issues",
            change: "index:a.ts",
        });

        // The list slice is additive; a view switch drops the selection and
        // keeps the slice.
        await dispatch({ type: "SET_ISSUES_TAB", data: "closed" });
        await dispatch({ type: "SET_VIEW", data: "graph" });
        expect(router.state.location.search).toEqual({
            view: "graph",
            issuesTab: "closed",
        });
        expect(captured.issuesTab).toBe("closed");
    });

    it("leaves the href alone for a no-op action", async () => {
        const router = await mountRepoPage(["/repo/5"]);
        const before = router.history.length;

        await dispatch({ type: "SET_VIEW", data: "overview" });

        expect(router.state.location.search).toEqual({});
        expect(router.history.length).toBe(before);
    });

    it("picks the state back up from a back navigation", async () => {
        const router = await mountRepoPage([
            "/repo/5",
            "/repo/5?view=graph&commit=abc123",
        ]);
        expect(captured.view).toBe("graph");

        router.history.back();
        await flush();

        expect(router.state.location.search).toEqual({});
        expect(captured.view).toBe("overview");
        expect(captured.selectedCommit).toBeNull();
    });
});
