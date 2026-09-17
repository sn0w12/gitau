import { Check, ListChecks, Pencil, Play, X } from "lucide-react";
import { useMemo, useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
    Popover,
    PopoverContent,
    PopoverTrigger,
} from "@/components/ui/popover";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import {
    useCommitHookRunner,
    useCommitHooks,
} from "@/hooks/repositories/use-commit-hooks";
import type { GitHook, HookRunResult } from "@/lib/backend/protocol";
import { cn } from "@/lib/utils";

import { ScrollArea } from "../../ui/scroll-area";
import { Tooltip, TooltipContent, TooltipTrigger } from "../../ui/tooltip";
import { HookEditorDialog } from "./hook-editor-dialog";

type RowState =
    | { phase: "running" }
    | { phase: "done"; result: HookRunResult }
    | { phase: "failed"; message: string };

interface ManualEntry {
    hook: string;
    result: HookRunResult;
}

interface LogEntry extends ManualEntry {
    source: "manual" | "commit";
}

const TRIGGER_CLASS = "relative text-muted-foreground hover:text-foreground";

/**
 * Pre-flight hook checker for the commit form: lists discovered
 * commit-lifecycle scripts, runs them on demand, shows captured output
 * with exit status per hook, and mirrors hook results from real commits
 * (`naturalRuns`). A manual rerun of the same hook supersedes its
 * committed-run entry.
 */
export function HookChecker({
    repoId,
    naturalRuns,
}: {
    repoId: number;
    naturalRuns?: HookRunResult[];
}) {
    const hooks = useCommitHooks(repoId);
    const { runs, log, busy, run, runAll } = useCommitHookRunner(repoId);
    const [activeHook, setActiveHook] = useState<string | null>(null);
    const [editorOpen, setEditorOpen] = useState(false);

    const entries = useMemo<LogEntry[]>(() => {
        const merged = new Map<string, LogEntry>();
        for (const result of naturalRuns ?? []) {
            merged.set(result.hook, {
                hook: result.hook,
                result,
                source: "commit",
            });
        }
        for (const entry of log as ManualEntry[]) {
            merged.set(entry.hook, { ...entry, source: "manual" });
        }
        return [...merged.values()];
    }, [log, naturalRuns]);

    const states = useMemo<Record<string, RowState>>(() => {
        const merged: Record<string, RowState> = { ...runs };
        for (const entry of entries) {
            if (!(entry.hook in merged)) {
                merged[entry.hook] = { phase: "done", result: entry.result };
            }
        }
        return merged;
    }, [runs, entries]);

    const discovered = hooks.data ?? [];
    const failed = entries.some((entry) => !entry.result.success);
    const active =
        activeHook != null
            ? (entries.find((entry) => entry.hook === activeHook) ?? null)
            : (entries.at(-1) ?? null);

    return (
        <Popover>
            <PopoverTrigger
                aria-label="Pre-commit hooks"
                data-failed={failed || undefined}
                render={
                    <Button
                        size="icon-xs"
                        variant="ghost"
                        className={cn(
                            TRIGGER_CLASS,
                            discovered.length > 0 && "text-primary"
                        )}
                    />
                }
            >
                <ListChecks className="size-3.5" />
                {entries.length > 0 && (
                    <span
                        aria-hidden
                        data-testid="hook-checker-status-dot"
                        className={`absolute top-0.5 right-0.5 size-1.5 rounded-full ${
                            failed ? "bg-destructive" : "bg-success"
                        }`}
                    />
                )}
            </PopoverTrigger>
            <PopoverContent
                align="center"
                side="top"
                className="w-120 overflow-hidden"
            >
                <div className="flex items-center justify-between px-1 pt-1">
                    <h2 className="text-sm font-semibold">Commit hooks</h2>
                    <div className="flex items-center gap-0.5">
                        {discovered.length > 0 && (
                            <Button
                                size="xs"
                                variant="ghost"
                                disabled={busy}
                                onClick={() => void runAll(discovered)}
                            >
                                Run all
                            </Button>
                        )}
                        <Button
                            size="icon-xs"
                            variant="ghost"
                            aria-label="Edit commit hooks"
                            onClick={() => setEditorOpen(true)}
                        >
                            <Pencil />
                        </Button>
                    </div>
                </div>
                <div>
                    {hooks.isPending ? (
                        <div className="space-y-1 px-1 py-1">
                            {[0, 1, 2].map((index) => (
                                <Skeleton key={index} className="h-5 w-full" />
                            ))}
                        </div>
                    ) : discovered.length === 0 ? (
                        <p
                            className="px-1 py-2 text-xs text-muted-foreground"
                            data-testid="hook-checker-empty"
                        >
                            No commit hooks installed in this repository.
                        </p>
                    ) : (
                        <ul className="space-y-0.5">
                            {discovered.map((hook) => (
                                <HookRow
                                    key={hook.name}
                                    hook={hook}
                                    state={states[hook.name]}
                                    onSelect={() => setActiveHook(hook.name)}
                                    onRun={() => {
                                        setActiveHook(hook.name);
                                        void run(hook.name);
                                    }}
                                />
                            ))}
                        </ul>
                    )}
                </div>
                {active && (
                    <div className="mt-auto border-t pt-1.5">
                        <div className="flex items-center gap-1 px-1 pb-1">
                            <Badge
                                variant={
                                    active.result.success ? "success" : "error"
                                }
                                size="sm"
                                className="font-mono"
                            >
                                {active.result.success
                                    ? "passed"
                                    : exitLabel(active.result.exitCode)}
                            </Badge>
                            <span className="min-w-0 truncate font-mono text-xs text-muted-foreground">
                                {active.source === "commit" && "committed · "}
                                {active.hook} · {active.result.durationMs} ms
                            </span>
                        </div>
                        <ScrollArea className="rounded-md bg-muted/40 [&_[data-slot=scroll-area-viewport]]:max-h-32">
                            <pre
                                data-testid="hook-checker-output"
                                className="p-1.5 font-mono text-xs leading-4 whitespace-pre-wrap"
                            >
                                {combinedOutput(
                                    active.result.stdout,
                                    active.result.stderr
                                ) || "(no output)"}
                            </pre>
                        </ScrollArea>
                    </div>
                )}
            </PopoverContent>
            <HookEditorDialog
                repoId={repoId}
                hooks={discovered}
                open={editorOpen}
                onClose={() => setEditorOpen(false)}
            />
        </Popover>
    );
}

function HookRow({
    hook,
    state,
    onSelect,
    onRun,
}: {
    hook: GitHook;
    state: RowState | undefined;
    onSelect: () => void;
    onRun: () => void;
}) {
    return (
        <li className="flex items-center rounded-md pr-0.5 pl-1 hover:bg-accent">
            <button
                type="button"
                className="flex min-w-0 flex-1 cursor-pointer items-center gap-1.5 py-1 text-left text-xs"
                onClick={onSelect}
            >
                <StatusGlyph state={state} />
                <span className="truncate font-mono">{hook.name}</span>
                {!hook.executable && (
                    <span className="shrink-0 text-muted-foreground">
                        not executable
                    </span>
                )}
                {state?.phase === "done" && (
                    <span className="ml-auto shrink-0 font-mono text-muted-foreground tabular-nums">
                        {state.result.durationMs} ms
                    </span>
                )}
            </button>
            <Button
                size="icon-2xs"
                variant="ghost"
                aria-label={`Run ${hook.name}`}
                disabled={state?.phase === "running"}
                onClick={(event) => {
                    event.stopPropagation();
                    onRun();
                }}
            >
                <Play />
            </Button>
        </li>
    );
}

function StatusGlyph({ state }: { state: RowState | undefined }) {
    if (!state) {
        return null;
    }
    switch (state.phase) {
        case "running":
            return <Spinner className="size-3 shrink-0" />;
        case "failed":
            return (
                <Tooltip>
                    <TooltipTrigger
                        render={
                            <span>
                                <X className="size-3 text-destructive" />
                            </span>
                        }
                    />
                    <TooltipContent>{state.message}</TooltipContent>
                </Tooltip>
            );
        case "done":
            return state.result.success ? (
                <Check className="size-3 shrink-0 text-success" />
            ) : (
                <X className="size-3 shrink-0 text-destructive" />
            );
    }
}

function exitLabel(exitCode: number | null): string {
    return exitCode == null ? "no exit code" : `exit ${exitCode}`;
}

function combinedOutput(stdout: string, stderr: string): string {
    return `${stdout}${stderr}`.trim();
}
