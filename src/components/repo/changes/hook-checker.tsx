import { Check, ListChecks, Pencil, Play, SkipForward, X } from "lucide-react";
import {
    useEffect,
    useLayoutEffect,
    useMemo,
    useRef,
    useState,
    type ComponentType,
} from "react";

import { HighlightedLine } from "@/components/diff/highlight-line";
import { Button } from "@/components/ui/button";
import {
    Collapsible,
    CollapsiblePanel,
    CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
    Popover,
    PopoverContent,
    PopoverTrigger,
} from "@/components/ui/popover";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import {
    type HookOutput,
    type HookRunState,
    outputOf,
    useCommitHookRunner,
    useCommitHooks,
} from "@/hooks/repositories/use-commit-hooks";
import type { GitHook, HookRunResult } from "@/lib/backend/protocol";
import { cn } from "@/lib/utils";

import { ScrollArea } from "../../ui/scroll-area";
import {
    TooltipCreateHandle,
    TooltipPayloadHost,
    TooltipProvider,
    TooltipTrigger,
    TooltipContent,
    Tooltip,
} from "../../ui/tooltip";
import { HookEditorDialog } from "./hook-editor-dialog";

interface LogEntry {
    hook: string;
    result: HookRunResult;
    source: "manual" | "commit";
}

const TRIGGER_CLASS = "relative text-muted-foreground hover:text-foreground";

/** How far from the bottom the viewport may sit and still follow along. */
const TAIL_SLACK_PX = 8;

const handle = TooltipCreateHandle<ComponentType>();
const runAllPayload = () => {
    return <span>Run All</span>;
};
const editPayload = () => {
    return <span>Edit Hooks</span>;
};

/**
 * Pre-flight hook checker for the commit form: lists discovered
 * commit-lifecycle scripts and runs them on demand. Each hook carries its
 * own collapsible output, streamed line by line while it runs with the
 * colours the hook wrote, and results from real commits (`naturalRuns`)
 * show the same way. A manual rerun of the same hook supersedes its
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
    const [opened, setOpened] = useState<Record<string, boolean>>({});
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
        for (const entry of log) {
            merged.set(entry.hook, { ...entry, source: "manual" });
        }
        return [...merged.values()];
    }, [log, naturalRuns]);

    const states = useMemo<Record<string, HookRunState>>(() => {
        const merged: Record<string, HookRunState> = { ...runs };
        for (const entry of entries) {
            if (!(entry.hook in merged)) {
                merged[entry.hook] = { phase: "done", result: entry.result };
            }
        }
        return merged;
    }, [runs, entries]);

    const discovered = hooks.data ?? [];
    const failed = entries.some((entry) => !entry.result.success);

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
                sideOffset={6}
                className="w-100 overflow-hidden"
            >
                <div className="flex items-center justify-between pt-1 pr-0.5 pl-1">
                    <h2 className="text-sm font-semibold">Commit hooks</h2>
                    <div className="flex items-center gap-0.5">
                        <TooltipProvider>
                            {discovered.length > 0 && (
                                <TooltipTrigger
                                    handle={handle}
                                    payload={runAllPayload}
                                    render={
                                        <Button
                                            size="icon-2xs"
                                            variant="ghost"
                                            disabled={busy}
                                            onClick={() =>
                                                void runAll(discovered)
                                            }
                                        >
                                            <SkipForward />
                                        </Button>
                                    }
                                />
                            )}
                            <TooltipTrigger
                                handle={handle}
                                payload={editPayload}
                                render={
                                    <Button
                                        size="icon-2xs"
                                        variant="ghost"
                                        aria-label="Edit commit hooks"
                                        onClick={() => setEditorOpen(true)}
                                    >
                                        <Pencil />
                                    </Button>
                                }
                            />
                            <TooltipPayloadHost handle={handle} />
                        </TooltipProvider>
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
                                    open={
                                        opened[hook.name] ??
                                        opensByDefault(states[hook.name])
                                    }
                                    onOpenChange={(next) =>
                                        setOpened((prev) => ({
                                            ...prev,
                                            [hook.name]: next,
                                        }))
                                    }
                                    onRun={() => void run(hook.name)}
                                />
                            ))}
                        </ul>
                    )}
                </div>
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

/** A hook shows its output while it runs and whenever it did not pass,
 * until the reader opens or closes it themselves. */
function opensByDefault(state: HookRunState | undefined): boolean {
    if (state?.phase === "running") return true;
    if (state?.phase === "failed") return true;
    return state?.phase === "done" && !state.result.success;
}

function HookRow({
    hook,
    state,
    open,
    onOpenChange,
    onRun,
}: {
    hook: GitHook;
    state: HookRunState | undefined;
    open: boolean;
    onOpenChange: (open: boolean) => void;
    onRun: () => void;
}) {
    return (
        <Collapsible render={<li />} open={open} onOpenChange={onOpenChange}>
            <div className="flex items-center rounded-md pr-0.5 pl-1 hover:bg-accent">
                <CollapsibleTrigger className="flex min-w-0 flex-1 cursor-pointer items-center gap-1.5 py-1 text-left text-xs">
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
                </CollapsibleTrigger>
                <Button
                    size="icon-2xs"
                    variant="ghost"
                    aria-label={`Run ${hook.name}`}
                    disabled={state?.phase === "running"}
                    onClick={onRun}
                >
                    <Play />
                </Button>
            </div>
            <CollapsiblePanel>
                <HookOutputPanel state={state} output={outputOf(state)} />
            </CollapsiblePanel>
        </Collapsible>
    );
}

function HookOutputPanel({
    state,
    output,
}: {
    state: HookRunState | undefined;
    output: HookOutput;
}) {
    const viewportRef = useRef<HTMLDivElement | null>(null);
    /** False once the reader has scrolled away from the tail; only their own
     * scrolling can set it, since following the tail counts as being there. */
    const pinnedRef = useRef(true);
    /** Offset the viewport is known to sit at, written both when the tail is
     * followed and when a scroll event reports a move. */
    const offsetRef = useRef(0);
    const mountedRef = useRef(false);
    const wasRunningRef = useRef(false);
    const running = state?.phase === "running";
    const { lines, styles } = output;

    useEffect(() => {
        const viewport = viewportRef.current;
        if (!viewport) return;
        const onScroll = () => {
            const offset = viewport.scrollTop;
            if (offset === offsetRef.current) return;
            offsetRef.current = offset;
            // Only a position the reader moved is measured against the tail.
            // A browser dispatches a scroll event on its own schedule, so one
            // raised by a follow can arrive after further output grew the
            // content past it; measuring then would read as the reader
            // scrolling away and stop a stream nobody took over.
            pinnedRef.current =
                viewport.scrollHeight - offset - viewport.clientHeight <
                TAIL_SLACK_PX;
        };
        viewport.addEventListener("scroll", onScroll);
        return () => viewport.removeEventListener("scroll", onScroll);
    }, []);

    useLayoutEffect(() => {
        const viewport = viewportRef.current;
        if (!viewport) return;
        const started = running && !wasRunningRef.current;
        wasRunningRef.current = running;
        const fresh = !mountedRef.current;
        mountedRef.current = true;
        // A panel opening on a finished run stays at the top, where a settled
        // log is read from, and starts following only once the reader scrolls
        // down. One that opens mid-run picks the stream up at the tail, and
        // so does a run started from an already open panel.
        if (fresh && !running) {
            pinnedRef.current = false;
            return;
        }
        if (started) {
            pinnedRef.current = true;
        }
        if (!pinnedRef.current) return;
        // Reading the offset back matters: the browser clamps the write to
        // the scrollable range, and that clamped value is what the scroll
        // event reports.
        viewport.scrollTop = viewport.scrollHeight;
        offsetRef.current = viewport.scrollTop;
    }, [lines.length, running]);

    return (
        <div className="mb-0.5 ml-4 rounded-md bg-muted/40">
            <ScrollArea
                viewportRef={viewportRef}
                className="[&_[data-slot=scroll-area-viewport]]:max-h-32"
            >
                <div
                    data-testid="hook-checker-output"
                    className="ui-selectable code-hl p-1.5 font-mono text-xs leading-4"
                >
                    {lines.length > 0 ? (
                        lines.map((line, index) => (
                            <div key={index} className="whitespace-pre-wrap">
                                <HighlightedLine
                                    code={line.text}
                                    row={
                                        line.spans.length > 0
                                            ? {
                                                  kind: "context",
                                                  content: line.text,
                                                  spans: line.spans,
                                              }
                                            : undefined
                                    }
                                    styles={styles}
                                />
                            </div>
                        ))
                    ) : (
                        <p className="text-muted-foreground">
                            {running ? "running" : "(no output)"}
                        </p>
                    )}
                </div>
            </ScrollArea>
        </div>
    );
}

function StatusGlyph({ state }: { state: HookRunState | undefined }) {
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
