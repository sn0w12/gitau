import { useQuery } from "@tanstack/react-query";
import { useCallback, useState } from "react";

import { useAppServices } from "@/contexts/services-context";
import { invalidateRepository } from "@/lib/backend/mutations/invalidation";
import type {
    CommitHookEvent,
    GitHook,
    HookOutputChunk,
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

/** The latest run of one hook, however it was started. A run in flight
 * carries the lines it has streamed so far. */
export type HookRunState =
    | { phase: "running"; output: HookOutput }
    | { phase: "done"; result: HookRunResult }
    | { phase: "failed"; message: string };

/** One entry per hook name, keyed by hook. */
export type HookRuns = Record<string, HookRunState>;

/** No runs yet, shared so a repo with none reads as a stable empty value. */
const NO_RUNS: HookRuns = {};

/** The runs a repo has produced, tagged so a switch to another repo does not
 * surface the previous repo's hook status. */
interface RepoRuns {
    repoId: number;
    runs: HookRuns;
}

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

/** What the checker renders, plus the two producers of a run. */
export interface HookRunner {
    runs: HookRuns;
    busy: boolean;
    run: (hook: string) => Promise<void>;
    runAll: (hooks: readonly GitHook[]) => Promise<void>;
    reportCommit: (event: CommitHookEvent) => void;
}

/**
 * Manual hook execution and the hooks a commit runs: every row runs
 * independently so one slow formatter never blocks checking another. Output
 * streams in line by line while the hook runs, and settles runs refresh
 * status, since linters and formatters may rewrite worktree contents outside
 * the backend's write path.
 *
 * The state lives here rather than inside the checker because both writers
 * have to reach it: the popover that starts a run, and the commit that runs
 * the pre-flight pipeline without being asked.
 */
export function useCommitHookRunner(repoId: number): HookRunner {
    const { backend, queryClient } = useAppServices();
    const [state, setState] = useState<RepoRuns>(() => ({
        repoId,
        runs: {},
    }));
    // Runs belong to the repo that produced them; another repo sees none
    // until it starts its own.
    const runs = state.repoId === repoId ? state.runs : NO_RUNS;

    const setRuns = useCallback(
        (fold: (runs: HookRuns) => HookRuns) => {
            setState((prev) => {
                const current = prev.repoId === repoId ? prev.runs : NO_RUNS;
                const next = fold(current);
                if (next === current && prev.repoId === repoId) return prev;
                return { repoId, runs: next };
            });
        },
        [repoId]
    );

    const begin = useCallback(
        (hook: string) =>
            setRuns((prev) => ({
                ...prev,
                [hook]: { phase: "running", output: NO_OUTPUT },
            })),
        [setRuns]
    );

    /** A line for a run in flight. A line that lands after the run settled
     * belongs to a run already reported, so it is dropped. */
    const append = useCallback(
        (hook: string, chunk: HookOutputChunk) =>
            setRuns((prev) => {
                const current = prev[hook];
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
            }),
        [setRuns]
    );

    const settle = useCallback(
        (hook: string, result: HookRunResult) =>
            setRuns((prev) => ({ ...prev, [hook]: { phase: "done", result } })),
        [setRuns]
    );

    const fail = useCallback(
        (hook: string, message: string) =>
            setRuns((prev) => ({
                ...prev,
                [hook]: { phase: "failed", message },
            })),
        [setRuns]
    );

    /** One event off a commit's hook channel. A commit is the other producer
     * of runs, so the checker sees a pre-commit nobody started by hand. */
    const reportCommit = useCallback(
        (event: CommitHookEvent) => {
            switch (event.type) {
                case "started":
                    begin(event.hook);
                    return;
                case "line":
                    append(event.hook, event);
                    return;
                case "settled":
                    settle(event.result.hook, event.result);
                    return;
            }
        },
        [append, begin, settle]
    );

    const run = useCallback(
        async (hook: string) => {
            begin(hook);
            const outcome = await backend.hooks.runStreamed(
                repoId,
                hook,
                (chunk) => append(hook, chunk)
            );
            if (!outcome.ok) {
                fail(hook, outcome.error.message);
                return;
            }
            settle(hook, outcome.value);
            await invalidateRepository(queryClient, repoId, ["status"]);
        },
        [append, backend, begin, fail, queryClient, repoId, settle]
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

    return { runs, busy, run, runAll, reportCommit };
}
