import { describe, expect, it, beforeEach, afterEach, vi } from "vitest";

import type { RepoSnapshot, SessionDocument } from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import {
    hydrateSession,
    initSessionPersistence,
    reopenSessionRepositories,
    scheduleReopenRetry,
} from "@/lib/bootstrap/session-persistence";
import { appStore, openTab, createTabRecord } from "@/stores/app-store";
import { repositoryStore, ensureRepo } from "@/stores/repository-store";

function fakeBackend(
    open: (
        path: string
    ) => Promise<
        | { ok: true; value: { id: number; snapshot: RepoSnapshot | null } }
        | { ok: false; error: { message: string } }
    >
): BackendClient {
    return { repositories: { open } } as unknown as BackendClient;
}

function resetStores() {
    appStore.setState(() => ({
        tabs: [],
        activeTabId: null,
    }));
    repositoryStore.setState(() => ({ entries: new Map() }));
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

describe("session restore", () => {
    beforeEach(resetStores);
    afterEach(() => {
        vi.useRealTimers();
    });

    it("keeps the repo path on tabs whose reopen failed", async () => {
        const backend = fakeBackend(async (path) =>
            path === "/gone"
                ? {
                      ok: false,
                      error: { message: "repositoryNotFound" },
                  }
                : { ok: true, value: { id: 7, snapshot: null } }
        );

        const session = {
            version: 1,
            tabs: [
                { tabId: "t1", title: "gone", repoPath: "/gone" },
                { tabId: "t2", title: "live", repoPath: "/live" },
            ],
            activeTabId: "t1",
        };

        const { idByPath, failedPaths } = await reopenSessionRepositories(
            session,
            backend
        );

        expect(failedPaths).toEqual(["/gone"]);
        expect(idByPath.get("/live")).toBe(7);

        hydrateSession(session);

        const [gone, live] = appStore.state.tabs;
        // The failed binding must survive: stripping it here would let the
        // debounced persistence erase the repository from session.json.
        expect(gone.repoPath).toBe("/gone");
        expect(live.repoPath).toBe("/live");
    });

    it("retry reopens failed repositories and binds them in the store", async () => {
        let failOpen = true;
        const backend = fakeBackend(async (_path) =>
            failOpen
                ? { ok: false, error: { message: "repositoryNotFound" } }
                : { ok: true, value: { id: 42, snapshot: null } }
        );

        const session = {
            version: 1,
            tabs: [{ tabId: "t1", title: "later", repoPath: "/slow" }],
            activeTabId: "t1",
        };

        const { failedPaths } = await reopenSessionRepositories(
            session,
            backend
        );
        hydrateSession(session);

        const cancel = scheduleReopenRetry(failedPaths, backend, 5);
        failOpen = false;
        await sleep(50);

        // Tabs resolve ids by path at runtime; the store carries the fresh
        // process-local binding after the retry succeeds.
        const entry = repositoryStore.state.entries.get("/slow");
        expect(entry?.repoId).toBe(42);
        expect(entry?.lastError).toBeUndefined();

        cancel();
    });

    it("cancel prevents the retry from firing", async () => {
        const open = vi.fn(async () => ({
            ok: true as const,
            value: { id: 1, snapshot: null },
        }));
        const backend = fakeBackend(open);

        const cancel = scheduleReopenRetry(["/x"], backend, 5);
        cancel();
        await sleep(30);

        expect(open).not.toHaveBeenCalled();
    });

    it("reopens added repositories that have no open tab", async () => {
        const opened: string[] = [];
        const backend = fakeBackend(async (path) => {
            opened.push(path);
            return { ok: true, value: { id: opened.length, snapshot: null } };
        });

        const session: SessionDocument = {
            version: 1,
            tabs: [], // No tabs at all. Repo was added but never tabbed.
            activeTabId: null,
            repositories: [
                { path: "/tabless-repo", addedAt: 1 },
                { path: "/other-repo", addedAt: 2 },
            ],
        };

        await reopenSessionRepositories(session, backend);
        hydrateSession(session);

        expect(opened).toEqual(
            expect.arrayContaining(["/tabless-repo", "/other-repo"])
        );
        const paths = [...repositoryStore.state.entries.values()].map(
            (entry) => entry.path
        );
        expect(paths).toEqual(
            expect.arrayContaining(["/tabless-repo", "/other-repo"])
        );
    });

    it("restores repos in the persisted order", async () => {
        const backend = fakeBackend(async () => ({
            ok: true,
            value: { id: 1, snapshot: null },
        }));
        const session: SessionDocument = {
            version: 1,
            tabs: [],
            activeTabId: null,
            repositories: [
                { path: "/third", addedAt: 3 },
                { path: "/first", addedAt: 1 },
                { path: "/second", addedAt: 2 },
            ],
        };

        await reopenSessionRepositories(session, backend);
        hydrateSession(session);

        const restored = [...repositoryStore.state.entries.values()].map(
            (entry) => entry.path
        );
        expect(restored).toEqual(["/third", "/first", "/second"]);
    });

    it("keeps a failed reopen listed with its error", async () => {
        const backend = fakeBackend(async (path) =>
            path === "/gone"
                ? { ok: false, error: { message: "missing" } }
                : { ok: true, value: { id: 1, snapshot: null } }
        );
        const session: SessionDocument = {
            version: 1,
            tabs: [],
            activeTabId: null,
            repositories: [{ path: "/gone", addedAt: 1 }],
        };

        await reopenSessionRepositories(session, backend);

        const entry = repositoryStore.state.entries.get("/gone");
        expect(entry?.lastError).toBe("missing");
    });

    it("seeds repos from tabs for documents persisted before repos existed", async () => {
        const backend = fakeBackend(async () => ({
            ok: true,
            value: { id: 5, snapshot: null },
        }));
        const legacy: SessionDocument = {
            version: 1,
            tabs: [{ tabId: "t1", title: "x", repoPath: "/legacy" }],
            activeTabId: "t1",
            // No `repositories` field.
        };

        await reopenSessionRepositories(legacy, backend);
        hydrateSession(legacy);

        expect(repositoryStore.state.entries.get("/legacy")?.repoId).toBe(5);
    });

    it("rewrites restored /repo/:id hrefs onto the fresh process ids", async () => {
        const backend = fakeBackend(async () => ({
            ok: true,
            value: { id: 99, snapshot: null },
        }));
        // Persisted last session: the repo had id 42 there; this run it is 99.
        const session: SessionDocument = {
            version: 1,
            tabs: [
                {
                    tabId: "t1",
                    title: "x",
                    repoPath: "/live",
                    lastResolvedHref: "/repo/42/commits",
                },
            ],
            activeTabId: "t1",
        };

        await reopenSessionRepositories(session, backend);
        hydrateSession(session, new Map([["/live", 99]]));

        expect(appStore.state.tabs[0].lastResolvedHref).toBe(
            "/repo/99/commits"
        );
    });
});

describe("session persistence subscription", () => {
    beforeEach(() => {
        resetStores();
        vi.useFakeTimers();
    });
    afterEach(() => {
        vi.useRealTimers();
    });

    it("debounces bursts into one save and flushes on dispose", async () => {
        const save = vi.fn(async (_doc: SessionDocument) => {});
        const dispose = initSessionPersistence(save);

        openTab(createTabRecord({ title: "one" }));
        openTab(createTabRecord({ title: "two" }));

        await vi.advanceTimersByTimeAsync(250);
        expect(save).toHaveBeenCalledTimes(1);
        expect(save.mock.calls[0][0].tabs).toHaveLength(2);

        dispose();
        expect(save).toHaveBeenCalledTimes(2);
    });

    it("persists known repos in the session document", async () => {
        const save = vi.fn(async (_doc: SessionDocument) => {});
        const dispose = initSessionPersistence(save);

        ensureRepo("/some/repo");
        ensureRepo("/SOME/repo"); // Case-insensitive dedupe.

        await vi.advanceTimersByTimeAsync(250);
        const doc = save.mock.calls.at(-1)![0];
        expect(doc.repositories).toEqual([
            { path: "/some/repo", addedAt: expect.any(Number) },
        ]);
        dispose();
    });

    it("persists repos in store order rather than by addedAt", async () => {
        const save = vi.fn(async (_doc: SessionDocument) => {});
        const dispose = initSessionPersistence(save);

        ensureRepo("/z-repo");
        ensureRepo("/a-repo");
        repositoryStore.setState((state) => {
            const entries = new Map(state.entries);
            entries.set("/z-repo", { ...entries.get("/z-repo")!, addedAt: 2 });
            entries.set("/a-repo", { ...entries.get("/a-repo")!, addedAt: 1 });
            return { entries };
        });

        await vi.advanceTimersByTimeAsync(250);
        const doc = save.mock.calls.at(-1)![0];
        expect(doc.repositories?.map((repo) => repo.path)).toEqual([
            "/z-repo",
            "/a-repo",
        ]);
        dispose();
    });
});
