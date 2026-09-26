"use no memo";

import {
    createRootRoute,
    createRoute,
    Outlet,
    redirect,
} from "@tanstack/react-router";

import { resolveTitleBadge } from "@/components/titlebar/title-badge-registry";
import {
    Empty,
    EmptyDescription,
    EmptyHeader,
    EmptyTitle,
} from "@/components/ui/empty";
import { TabContext } from "@/contexts/tab-context";
import { repoDisplayName } from "@/hooks/repositories/use-repo-identity";
import {
    remotesQuery,
    snapshotQuery,
    statusQuery,
} from "@/lib/backend/queries/repository-queries";
import { getAppRuntime } from "@/lib/bootstrap/app-runtime";
import { openRepositoryByPath } from "@/lib/repositories/open-repository";
import { AccountPage } from "@/routes/account-page";
import { DevPage } from "@/routes/dev-page";
import { InboxPage } from "@/routes/inbox-page";
import { RepoPage } from "@/routes/repo-page";
import { SettingsPage } from "@/routes/settings-page";
import { setTabTitle } from "@/stores/app-store";
import { getEntryByRepoId } from "@/stores/repository-store";

import { HomePage } from "./home-page";
import { IssuePage } from "./repo/issue";

/**
 * Fresh route-tree instances per call: routers mutate their route nodes
 * internally, so trees are never shared between live routers.
 *
 * Route loaders write each tab's title/badge into the app store, so restored
 * tabs hydrate before first paint; badges then self-feed via react-query,
 * keeping inactive tabs' titles live without mounting the page.
 */
export function createTabRouteTree(options: { tabId?: string } = {}) {
    const tabId = options.tabId ?? null;

    const rootRoute = createRootRoute({
        component: () => (
            <TabContext.Provider value={tabId}>
                <Outlet />
            </TabContext.Provider>
        ),
        notFoundComponent: () => (
            <div className="size-full">
                <Empty>
                    <EmptyHeader>
                        <EmptyTitle>Not Found</EmptyTitle>
                        <EmptyDescription>
                            This page was not found
                        </EmptyDescription>
                    </EmptyHeader>
                </Empty>
            </div>
        ),
    });

    const applyTitle = (title: string, badgeKey?: string) => {
        // Trees built without a tab id belong to non-tab routers.
        if (tabId === null) return;
        setTabTitle(tabId, title, resolveTitleBadge(badgeKey));
    };

    const homeRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/",
        component: HomePage,
        loader: () => applyTitle("Home"),
    });

    const settingsRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/settings",
        component: SettingsPage,
        loader: () => applyTitle("Settings"),
    });

    const accountRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/account",
        component: AccountPage,
        loader: () => applyTitle("Account"),
    });

    const inboxRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/inbox",
        component: InboxPage,
        loader: () => applyTitle("Inbox"),
    });

    // Playground for mocking features; never shipped to release builds.
    const devRoute = import.meta.env.DEV
        ? createRoute({
              getParentRoute: () => rootRoute,
              path: "/dev",
              component: DevPage,
              loader: () => applyTitle("Dev"),
          })
        : null;

    const repoRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/repo/$repoId",
        component: RepoPage,
        validateSearch: (search: Record<string, unknown>) => ({
            view:
                search.view === "issues" || search.view === "graph"
                    ? search.view
                    : undefined,
        }),
        loader: async ({ params }) => {
            const routeId = Number(params.repoId);
            if (!Number.isInteger(routeId) || routeId <= 0) return;

            const known = getEntryByRepoId(routeId);
            const path = known?.path;
            const displayName = path ? repoDisplayName(path) : "Repository";
            applyTitle(displayName, path ? "repo" : undefined);

            // Guarantee the backend binding: page + badge queries need a
            // process-local id to fetch anything.
            let boundId = known?.repoId;
            if (boundId === undefined && path) {
                const result = await openRepositoryByPath(path);
                if (result.status === "failed") return;
                boundId = result.repoId;
            }
            if (boundId === undefined) return;

            // A restored href can embed a previous process-local id; send
            // the router to this session's real one.
            if (boundId !== routeId) {
                throw redirect({
                    to: "/repo/$repoId",
                    params: { repoId: String(boundId) },
                    replace: true,
                });
            }

            // Warm the queries the badge and page both read. Status starts
            // here too: the backend warms it on open, so the first changes
            // panel paint hits the cache.
            const { backend, queryClient } = getAppRuntime();
            void queryClient.prefetchQuery(snapshotQuery({ backend }, boundId));
            void queryClient.prefetchQuery(remotesQuery({ backend }, boundId));
            void queryClient.prefetchQuery(statusQuery({ backend }, boundId));
        },
    });

    const repoIssueRoute = createRoute({
        getParentRoute: () => rootRoute,
        path: "/repo/$repoId/issue/$issueId",
        component: IssuePage,
        loader: () => applyTitle("Issue", "issue"),
    });

    return rootRoute.addChildren([
        homeRoute,
        settingsRoute,
        accountRoute,
        inboxRoute,
        ...(devRoute ? [devRoute] : []),
        repoRoute,
        repoIssueRoute,
    ]);
}
