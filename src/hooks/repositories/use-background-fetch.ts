import { useSelector } from "@tanstack/react-store";
import { useEffect, useRef } from "react";

import { useAppServices } from "@/contexts/services-context";
import { invalidateRepository } from "@/lib/backend/mutations/invalidation";
import { primaryRemoteName } from "@/lib/repositories/fetch-action";
import {
    markFetchEnded,
    markFetchStarted,
    markRepositoryFetched,
} from "@/stores/fetch-store";
import { repositoryStore } from "@/stores/repository-store";

/** GitHub Desktop's default interval (1h) for silently refreshing repos. */
export const DEFAULT_FEED_INTERVAL_MS = 60 * 60 * 1000;
/** Floor so a misbehaving server can never make us fetch faster than this. */
export const MINIMUM_INTERVAL_MS = 5 * 60 * 1000;
/** Upper bound on the random stagger applied to each repo's interval. */
export const MAX_SKEW_MS = 30 * 1000;

function intervalWithSkew(): number {
    return Math.max(
        DEFAULT_FEED_INTERVAL_MS + Math.floor(Math.random() * MAX_SKEW_MS),
        MINIMUM_INTERVAL_MS
    );
}

interface OpenRepo {
    path: string;
    repoId: number;
}

function selectOpenRepos(state: {
    entries: Map<string, { path: string; repoId?: number }>;
}): OpenRepo[] {
    const out: OpenRepo[] = [];
    for (const [path, entry] of state.entries) {
        if (entry.repoId !== undefined) {
            out.push({ path, repoId: entry.repoId });
        }
    }
    return out;
}

/**
 * Mirrors GitHub Desktop's BackgroundFetcher: while a repository is open, run
 * one quiet fetch per repo on a staggered hourly timer and record it as the
 * "last fetched" time. Fetch errors are swallowed (never toasted) because
 * this runs unattended. One timer chain per repo keeps their schedules
 * independent of tab switches.
 */
export function useBackgroundFetch(): void {
    const { backend, queryClient } = useAppServices();

    // repoId -> chain handle; keeps schedules alive between re-renders.
    const chainsRef = useRef(new Map<number, ReturnType<typeof setTimeout>>());
    // repoId -> an in-flight background fetch (avoid self-overlap).
    const inflightRef = useRef(new Set<number>());

    const openRepos = useSelector(repositoryStore, selectOpenRepos);

    useEffect(() => {
        const chains = chainsRef.current;
        const inflight = inflightRef.current;

        const chainFor = (repoId: number) => chains.get(repoId);

        const isRepoOpen = (repoId: number) =>
            selectOpenRepos(repositoryStore.state).some(
                (repo) => repo.repoId === repoId
            );

        // One chained timer per repo: tick, fetch, then schedule the next.
        const schedule = (
            repo: OpenRepo,
            delayMs: number,
            disposePrevious: boolean
        ) => {
            const previous = chainFor(repo.repoId);
            if (disposePrevious && previous !== undefined) {
                clearTimeout(previous);
            }

            const handle = setTimeout(() => {
                void tick(repo);
            }, delayMs);
            chains.set(repo.repoId, handle);
        };

        const tick = async (repo: OpenRepo) => {
            if (!isRepoOpen(repo.repoId)) {
                chains.delete(repo.repoId);
                return;
            }
            if (inflight.has(repo.repoId)) {
                // A fetch is still running; retry on the next slot.
                schedule(repo, intervalWithSkew(), true);
                return;
            }
            inflight.add(repo.repoId);
            markFetchStarted(repo.path);
            try {
                const remotes = await backend.remotes.list(repo.repoId);
                const remoteName = remotes.ok
                    ? primaryRemoteName(remotes.value)
                    : undefined;
                if (remoteName) {
                    const result = await backend.remotes.fetch(repo.repoId, {
                        remote: remoteName,
                        prune: true,
                    });
                    if (result.ok) {
                        markRepositoryFetched(repo.path);
                        await invalidateRepository(queryClient, repo.repoId, [
                            "refs",
                            "history",
                            "remotes",
                        ]);
                    }
                }
            } finally {
                inflight.delete(repo.repoId);
                markFetchEnded(repo.path);
            }
            if (isRepoOpen(repo.repoId)) {
                schedule(repo, intervalWithSkew(), true);
            } else {
                chains.delete(repo.repoId);
            }
        };

        const openIds = new Set(openRepos.map((repo) => repo.repoId));
        // Stop schedules for repos that closed, then start new ones.
        for (const repoId of chains.keys()) {
            if (!openIds.has(repoId)) {
                clearTimeout(chainFor(repoId));
                chains.delete(repoId);
                inflight.delete(repoId);
            }
        }
        for (const repo of openRepos) {
            if (chainFor(repo.repoId) === undefined) {
                schedule(repo, intervalWithSkew(), false);
            }
        }

        return () => {
            for (const handle of chains.values()) clearTimeout(handle);
            chains.clear();
            inflight.clear();
        };
    }, [backend, queryClient, openRepos]);
}
