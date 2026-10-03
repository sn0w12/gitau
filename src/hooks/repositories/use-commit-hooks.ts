import { useQuery } from "@tanstack/react-query";
import { useCallback, useState } from "react";

import { useAppServices } from "@/contexts/services-context";
import { invalidateRepository } from "@/lib/backend/mutations/invalidation";
import type {
    GitHook,
    HookOutputLine,
    HookRunResult,
    SyntaxStyle,
} from "@/lib/backend/protocol";
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

/** Output of a run, live or settled, in the shape the rows render. */
export interface HookOutput {
    lines: HookOutputLine[];
    styles: SyntaxStyle[];
}

const NO_OUTPUT: HookOutput = { lines: [], styles: [] };

/** One settled (or in-flight) manual run per hook name. A run in flight
 * carries the lines it has streamed so far. */
export type HookRunState =
    | { phase: "running"; output: HookOutput }
    | { phase: "done"; result: HookRunResult }
    | { phase: "failed"; message: string };

/** What a run has to show: a live run its streamed lines, a settled one the
 * result the backend parsed the same way. */
export function outputOf(state: HookRunState | undefined): HookOutput {
    if (state == null) return NO_OUTPUT;
    return state.phase === "running"
        ? state.output
        : state.phase === "done"
          ? { lines: state.result.lines, styles: state.result.styles }
          : NO_OUTPUT;
}

/**
 * Manual hook execution: every row runs independently so one slow
 * formatter never blocks checking another. Output streams in line by line
 * while the hook runs, and settles runs refresh status, since linters and
 * formatters may rewrite worktree contents outside the backend's write path.
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
            setRuns((prev) => ({
                ...prev,
                [hook]: { phase: "running", output: NO_OUTPUT },
            }));
            const outcome = await backend.hooks.runStreamed(
                repoId,
                hook,
                (chunk) => {
                    setRuns((prev) => {
                        const current = prev[hook];
                        // A line that lands after the run settled belongs to a
                        // run already reported, so it is dropped.
                        if (current?.phase !== "running") return prev;
                        return {
                            ...prev,
                            [hook]: {
                                phase: "running",
                                output: {
                                    lines: [
                                        ...current.output.lines,
                                        {
                                            text: chunk.text,
                                            spans: chunk.spans,
                                        },
                                    ],
                                    styles: [
                                        ...current.output.styles,
                                        ...(chunk.styles ?? []),
                                    ],
                                },
                            },
                        };
                    });
                }
            );
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
