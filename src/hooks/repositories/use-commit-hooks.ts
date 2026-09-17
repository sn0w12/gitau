import { useQuery } from "@tanstack/react-query";
import { useCallback, useState } from "react";

import { useAppServices } from "@/contexts/services-context";
import { invalidateRepository } from "@/lib/backend/mutations/invalidation";
import type { GitHook, HookRunResult } from "@/lib/backend/protocol";
import { hooksQuery } from "@/lib/backend/queries/repository-queries";

/** Placeholder ids never reach the backend; pages mount before repo
 * binding resolves. */
function hasValidRepoId(repoId: number | undefined): boolean {
    return typeof repoId === "number" && Number.isInteger(repoId) && repoId > 0;
}

/**
 * Lists discovered commit-lifecycle hook scripts (`core.hooksPath` aware);
 * standard hooks absent on disk simply do not appear.
 */
export function useCommitHooks(repoId: number | undefined) {
    const { backend } = useAppServices();
    return useQuery({
        ...hooksQuery({ backend }, repoId ?? 0),
        enabled: hasValidRepoId(repoId),
    });
}

/** One settled (or in-flight) manual run per hook name. */
export type HookRunState =
    | { phase: "running" }
    | { phase: "done"; result: HookRunResult }
    | { phase: "failed"; message: string };

/**
 * Manual hook execution: every row runs independently so one slow
 * formatter never blocks checking another. Settled runs refresh status,
 * since linters and formatters may rewrite worktree contents outside the
 * backend's write path.
 */
export function useCommitHookRunner(repoId: number) {
    const { backend, queryClient } = useAppServices();
    const [runs, setRuns] = useState<Record<string, HookRunState>>({});
    // Insertion-ordered record of the latest result per hook.
    const [log, setLog] = useState<{ hook: string; result: HookRunResult }[]>(
        []
    );

    const run = useCallback(
        async (hook: string) => {
            setRuns((prev) => ({ ...prev, [hook]: { phase: "running" } }));
            const outcome = await backend.hooks.run(repoId, hook);
            if (!outcome.ok) {
                setRuns((prev) => ({
                    ...prev,
                    [hook]: { phase: "failed", message: outcome.error.message },
                }));
                return;
            }
            const result = outcome.value;
            setRuns((prev) => ({
                ...prev,
                [hook]: { phase: "done", result },
            }));
            setLog((prev) =>
                prev.some((entry) => entry.hook === hook)
                    ? prev.map((entry) =>
                          entry.hook === hook ? { hook, result } : entry
                      )
                    : [...prev, { hook, result }]
            );
            await invalidateRepository(queryClient, repoId, ["status"]);
        },
        [backend, queryClient, repoId]
    );

    /** Sequential in pipeline order so output reads like a real commit. */
    const runAll = useCallback(
        async (hooks: readonly GitHook[]) => {
            for (const hook of hooks) {
                await run(hook.name);
            }
        },
        [run]
    );

    const busy = Object.values(runs).some((state) => state.phase === "running");

    return { runs, log, busy, run, runAll };
}
