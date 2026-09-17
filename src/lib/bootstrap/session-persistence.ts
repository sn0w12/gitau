import { resolveTitleBadge } from "@/components/titlebar/title-badge-registry";
import type { SessionDocument } from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import { getAppRuntime } from "@/lib/bootstrap/app-runtime";
import { appStore, replaceTabs } from "@/stores/app-store";
import type { TabRecord } from "@/stores/app-store";
import { fetchStore, hydrateFetchTimestamps } from "@/stores/fetch-store";
import {
    ensureRepo,
    registerOpen,
    repositoryStore,
    setEntryError,
} from "@/stores/repository-store";

const PERSIST_DEBOUNCE_MS = 250;

export const REOPEN_RETRY_DELAY_MS = 10_000;

export interface ReopenResult {
    /** Successfully opened repositories: path -> fresh process-local id. */
    idByPath: Map<string, number>;
    /** Paths whose open failed; they stay bound to their tabs so a
     * transient failure doesn't erase the persisted binding. */
    failedPaths: string[];
}

/**
 * Reopens every repo referenced by the session (by path) and reports
 * path->id mappings plus failures. Backend repoIds are process-local, so
 * persisted tab bindings must be remapped through this after restore.
 */
export async function reopenSessionRepositories(
    session: SessionDocument,
    backend?: BackendClient
): Promise<ReopenResult> {
    // Reopen explicitly added repos even when no tab references them, plus
    // any tab-bound paths (covers docs persisted before repos were tracked).
    const paths = [
        ...new Set(
            [
                ...(session.repositories?.map((repo) => repo.path) ?? []),
                ...session.tabs.map((tab) => tab.repoPath),
            ].filter((path): path is string => !!path)
        ),
    ];

    const client = backend ?? getAppRuntime().backend;

    const idByPath = new Map<string, number>();
    const failedPaths: string[] = [];
    await Promise.all(
        paths.map(async (repoPath) => {
            // List the repo immediately, even before its open resolves, so
            // failed paths stay visible and retryable instead of vanishing.
            ensureRepo(repoPath);
            const opened = await client.repositories.open(repoPath);
            if (!opened.ok) {
                console.warn(
                    `could not reopen repository "${repoPath}":`,
                    opened.error.message
                );
                setEntryError(repoPath, opened.error.message);
                failedPaths.push(repoPath);
                return;
            }
            registerOpen(repoPath, opened.value.id);
            idByPath.set(repoPath, opened.value.id);
        })
    );
    return { idByPath, failedPaths };
}

/**
 * Retries repos that failed to reopen once, after a delay (network drives
 * and USB volumes often come online shortly after login). Returns a cancel
 * function for tests/teardown.
 */
export function scheduleReopenRetry(
    failedPaths: string[],
    backend: BackendClient,
    delayMs: number = REOPEN_RETRY_DELAY_MS
): () => void {
    if (failedPaths.length === 0) return () => {};
    const timer = setTimeout(() => {
        void (async () => {
            await Promise.all(
                failedPaths.map(async (repoPath) => {
                    const opened = await backend.repositories.open(repoPath);
                    if (!opened.ok) {
                        console.warn(
                            `retry reopening repository "${repoPath}" failed:`,
                            opened.error.message
                        );
                        return;
                    }
                    registerOpen(repoPath, opened.value.id);
                })
            );
        })();
    }, delayMs);
    return () => clearTimeout(timer);
}

/** Seeds the app store from a restored session document. */
export function hydrateSession(
    session: SessionDocument,
    idByPath: Map<string, number> = new Map()
): void {
    hydrateFetchTimestamps(session.lastFetched);
    if (!Array.isArray(session.tabs)) return;

    // Restored hrefs may embed a previous process-local repo id; rewrite
    // them onto the freshly opened ids.
    const rewriteHref = (href: string | undefined, path?: string): string => {
        const base = href ?? "/";
        const freshId = path ? idByPath.get(path) : undefined;
        if (freshId === undefined || !/^\/repo\/\d+/.test(base)) return base;
        return base.replace(/^\/repo\/\d+/, `/repo/${freshId}`);
    };

    const tabs: TabRecord[] = session.tabs.map((tab) => ({
        tabId: tab.tabId,
        title: tab.title,
        lastResolvedHref: rewriteHref(tab.lastResolvedHref, tab.repoPath),
        // The path is the durable binding; persisted ids never survive a
        // restart. Badges are self-resolving, so restoring them is safe.
        repoPath: tab.repoPath || undefined,
        titleBadge: resolveTitleBadge(tab.titleBadge),
        createdAt: Date.now(),
    }));

    const activeTabId =
        session.activeTabId != null &&
        tabs.some((tab) => tab.tabId === session.activeTabId)
            ? session.activeTabId
            : (tabs[tabs.length - 1]?.tabId ?? null);

    replaceTabs(tabs, activeTabId);

    // Ensure every persisted repo path exists in the store even if its open
    // is still pending or failed. The list must never shrink on restart.
    for (const path of [
        ...(session.repositories?.map((repo) => repo.path) ?? []),
        ...session.tabs.map((tab) => tab.repoPath),
    ]) {
        if (path) ensureRepo(path);
    }
}

/** Snapshot of the current app state as a persistable document. */
export function currentSessionDocument(): SessionDocument {
    const { tabs, activeTabId } = appStore.state;
    return {
        version: 1,
        tabs: tabs.map((tab) => ({
            tabId: tab.tabId,
            title: tab.title,
            lastResolvedHref: tab.lastResolvedHref,
            repoPath: tab.repoPath,
            titleBadge: tab.titleBadge?.key,
        })),
        activeTabId,
        lastFetched: { ...fetchStore.state.lastFetchedByPath },
        // The array order IS the repo order; the sidebar and homepage
        // render the store's insertion order directly.
        repositories: [...repositoryStore.state.entries.values()].map(
            (entry) => ({ path: entry.path, addedAt: entry.addedAt })
        ),
    };
}

/**
 * Keeps storage in sync with the app stores (debounced). Save failures are
 * surfaced loudly; silent data loss is worse than noise. Returns an
 * unsubscribe for tests/teardown.
 */
export function initSessionPersistence(
    save: (doc: SessionDocument) => Promise<void>
): () => void {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const schedule = () => {
        clearTimeout(timer);
        timer = setTimeout(() => {
            void save(currentSessionDocument()).catch((error: unknown) => {
                console.error("session save failed:", error);
            });
        }, PERSIST_DEBOUNCE_MS);
    };
    // Repo additions/removals must persist too, not just tab mutations.
    const subscriptions = [
        appStore.subscribe(schedule),
        repositoryStore.subscribe(schedule),
    ];

    return () => {
        clearTimeout(timer);
        for (const subscription of subscriptions) subscription.unsubscribe();
        void save(currentSessionDocument()).catch((error: unknown) => {
            console.error("final session save failed:", error);
        });
    };
}
