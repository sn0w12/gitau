import {
    Ban,
    Check,
    CheckCheck,
    ChevronDown,
    CircleAlert,
    CircleDashed,
    CircleSlash,
    Info,
    TriangleAlert,
    X,
} from "lucide-react";
import { useMemo, useState } from "react";

import { HighlightedLine } from "@/components/diff/highlight-line";
import { CustomMarkdown } from "@/components/github/markdown";
import { SidebarBlock } from "@/components/github/thread-chrome";
import { Button } from "@/components/ui/button";
import {
    Collapsible,
    CollapsiblePanel,
    CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
    Dialog,
    DialogClose,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogPanel,
    DialogPopup,
    DialogTitle,
} from "@/components/ui/dialog";
import { Frame, FrameHeader, FramePanel } from "@/components/ui/frame";
import { Skeleton } from "@/components/ui/skeleton";
import { toastManager } from "@/components/ui/toast";
import { useAppServices } from "@/contexts/services-context";
import {
    useCheckRun,
    useCheckRunLog,
} from "@/hooks/github/use-github-pull-requests";
import { useCopyToClipboard } from "@/hooks/use-copy-to-clipboard";
import type {
    GithubActionStep,
    GithubCheckAnnotation,
    GithubCheckRun,
    GithubCheckRunOutput,
    GithubCommitStatus,
    GithubWorkflowRun,
    SyntaxStyle,
} from "@/lib/backend/protocol";
import { toastError } from "@/lib/toast-error";
import { cn } from "@/lib/utils";
function checkAppearance(run: GithubCheckRun): {
    icon: React.ReactNode;
    className: string;
    label: string;
} {
    const conclusion = run.conclusion?.toLowerCase() ?? "";
    if (run.status !== "completed") {
        return {
            icon: <CircleDashed className="size-4" />,
            className: "text-info",
            label: run.status === "queued" ? "Queued" : "Running",
        };
    }
    if (conclusion === "success") {
        return {
            icon: <CheckCheck className="size-4" />,
            className: "text-success",
            label: "Passed",
        };
    }
    if (conclusion === "skipped" || conclusion === "neutral") {
        return {
            icon: <CircleSlash className="size-4" />,
            className: "text-muted-foreground",
            label: conclusion === "skipped" ? "Skipped" : "Neutral",
        };
    }
    if (conclusion === "action_required") {
        return {
            icon: <TriangleAlert className="size-4" />,
            className: "text-warning",
            label: "Action required",
        };
    }
    if (conclusion === "cancelled") {
        return {
            icon: <Ban className="size-4" />,
            className: "text-muted-foreground",
            label: "Cancelled",
        };
    }
    if (conclusion === "stale") {
        return {
            icon: <TriangleAlert className="size-4" />,
            className: "text-warning",
            label: "Stale",
        };
    }
    return {
        icon: <X className="size-4" />,
        className: "text-destructive",
        label: "Failed",
    };
}

interface CheckRow {
    key: string;
    name: string;
    checkRunId?: number;
    /** Commit statuses carry their own text, e.g. `Review in progress`. */
    detail?: string;
    /** Whether there is a run to open. A commit status has none, and a
     * skipped check has no job, so those are plain rows. */
    interactive: boolean;
    appearance: { icon: React.ReactNode; className: string; label: string };
}

/** A commit status reports one of four states, and `pending` is the reporter
 * saying it has not finished, which is what GitHub shows as waiting. */
function statusAppearance(status: GithubCommitStatus): CheckRow["appearance"] {
    switch (status.state.toLowerCase()) {
        case "success":
            return {
                icon: <CheckCheck className="size-4" />,
                className: "text-success",
                label: "Passed",
            };
        case "pending":
            return {
                icon: <CircleDashed className="size-4" />,
                className: "text-info",
                label: "Pending",
            };
        case "error":
            return {
                icon: <TriangleAlert className="size-4" />,
                className: "text-warning",
                label: "Error",
            };
        default:
            return {
                icon: <Check className="size-4" />,
                className: "text-destructive",
                label: "Failed",
            };
    }
}

/** A check run's details URL carries the workflow run it belongs to
 * (`/actions/runs/{run}/job/{job}`), which is the only reliable way to tell
 * that a workflow run and a check run are the same work: their names differ,
 * since the check is named after the job. */
function workflowRunIdOf(detailsUrl: string | null | undefined) {
    if (detailsUrl == null) return null;
    return /\/actions\/runs\/(\d+)/.exec(detailsUrl)?.[1] ?? null;
}

function useCheckRows(
    checks: GithubCheckRun[],
    workflows: GithubWorkflowRun[],
    statuses: GithubCommitStatus[]
): CheckRow[] {
    return useMemo(() => {
        const covered = new Set(
            checks
                .map((check) => workflowRunIdOf(check.detailsUrl))
                .filter((id): id is string => id != null)
        );
        // A reporter that has both a check run and a status under the same
        // name is one piece of work, and GitHub's own list merges them.
        const named = new Set(
            [...checks, ...workflows].map((run) => run.name.toLowerCase())
        );
        return [
            ...checks.map((check) => ({
                key: `check-${check.id}`,
                name: check.name,
                checkRunId: check.id,
                interactive: check.conclusion?.toLowerCase() !== "skipped",
                appearance: checkAppearance(check),
            })),
            // Workflow runs only add runs the checks do not already cover, so
            // one piece of CI does not show up twice.
            ...workflows
                .filter((run) => !covered.has(String(run.id)))
                .map((run) => ({
                    key: `workflow-${run.id}`,
                    name: run.name,
                    interactive: true,
                    appearance: workflowAppearance(run),
                })),
            ...statuses
                .filter((status) => !named.has(status.context.toLowerCase()))
                .map((status) => ({
                    key: `status-${status.id}`,
                    name: status.context,
                    detail: status.description,
                    interactive: false,
                    appearance: statusAppearance(status),
                })),
        ];
    }, [checks, workflows, statuses]);
}

export function ChecksSection({
    owner,
    repo,
    checks,
    workflows,
    statuses,
    isLoading,
}: {
    owner: string;
    repo: string;
    checks: GithubCheckRun[];
    workflows: GithubWorkflowRun[];
    statuses: GithubCommitStatus[];
    isLoading: boolean;
}) {
    const runs = useCheckRows(checks, workflows, statuses);
    const [open, setOpen] = useState<CheckRow | null>(null);

    if (isLoading) {
        return (
            <SidebarBlock label="Checks">
                <div className="flex flex-col gap-1.5" aria-busy="true">
                    <Skeleton className="h-4 w-4/5" />
                    <Skeleton className="h-4 w-3/5" />
                </div>
            </SidebarBlock>
        );
    }
    if (runs.length === 0) {
        return (
            <SidebarBlock label="Checks">
                <span className="text-sm text-muted-foreground">None</span>
            </SidebarBlock>
        );
    }
    return (
        <SidebarBlock label="Checks">
            <ul className="flex flex-col gap-0.5">
                {runs.map((run) => (
                    <li key={run.key} className="text-sm">
                        {run.interactive ? (
                            <button
                                type="button"
                                className="ui-selectable flex w-full items-center gap-1.5 rounded-sm text-start hover:bg-accent"
                                onClick={() => setOpen(run)}
                            >
                                <CheckRowBody run={run} />
                            </button>
                        ) : (
                            <div className="ui-selectable flex w-full items-center gap-1.5 rounded-sm text-start">
                                <CheckRowBody run={run} />
                            </div>
                        )}
                    </li>
                ))}
            </ul>
            <CheckRunDialog
                owner={owner}
                repo={repo}
                run={open}
                onClose={() => setOpen(null)}
            />
        </SidebarBlock>
    );
}

/** One line of the list: state glyph, name with any reporter text, and the
 * outcome on the right. */
function CheckRowBody({ run }: { run: CheckRow }) {
    return (
        <>
            <span className={run.appearance.className}>
                {run.appearance.icon}
            </span>
            <span className="truncate">
                {run.name}{" "}
                {run.detail ? (
                    <span className="text-muted-foreground">{run.detail}</span>
                ) : null}
            </span>
            <span className="ms-auto shrink-0 text-xs text-muted-foreground">
                {run.appearance.label}
            </span>
        </>
    );
}

function CheckRunDialog({
    owner,
    repo,
    run,
    onClose,
}: {
    owner: string;
    repo: string;
    run: CheckRow | null;
    onClose: () => void;
}) {
    return (
        <Dialog
            open={run != null}
            onOpenChange={(next) => {
                if (!next) onClose();
            }}
        >
            <DialogPopup className="h-[80vh] max-w-4xl">
                {run === null ? null : (
                    <CheckRunDialogBody owner={owner} repo={repo} run={run} />
                )}
            </DialogPopup>
        </Dialog>
    );
}

function CheckRunDialogBody({
    owner,
    repo,
    run,
}: {
    owner: string;
    repo: string;
    run: CheckRow;
}) {
    const detail = useCheckRun(owner, repo, run.checkRunId ?? null);
    const output = detail.data?.output;
    const annotations = detail.data?.annotations ?? [];
    return (
        <>
            <DialogHeader>
                <DialogTitle>{run.name}</DialogTitle>
                <DialogDescription className="flex items-center gap-1.5">
                    <span className={run.appearance.className}>
                        {run.appearance.icon}
                    </span>
                    {run.appearance.label}
                </DialogDescription>
            </DialogHeader>
            <DialogPanel
                fill
                className="ui-selectable flex flex-col gap-3 text-sm"
            >
                {detail.isPending ? (
                    <JobLogSkeleton />
                ) : detail.isError ? (
                    <p className="text-destructive">
                        Could not load these results.
                    </p>
                ) : (
                    <CheckRunResults
                        owner={owner}
                        repo={repo}
                        run={run}
                        output={output}
                        annotations={annotations}
                    />
                )}
            </DialogPanel>
            <DialogFooter>
                <DialogClose render={<Button variant="ghost" />}>
                    Close
                </DialogClose>
            </DialogFooter>
        </>
    );
}

function CheckRunResults({
    owner,
    repo,
    run,
    output,
    annotations,
}: {
    owner: string;
    repo: string;
    run: CheckRow;
    output: GithubCheckRunOutput | undefined;
    annotations: GithubCheckAnnotation[];
}) {
    const log = useCheckRunLog(owner, repo, run.checkRunId ?? null, true);
    const hasSummary = (output?.summary.trim().length ?? 0) > 0;
    return (
        <>
            {output?.title ? (
                <p className="font-medium">{output.title}</p>
            ) : null}
            {hasSummary ? (
                <CustomMarkdown>{output?.summary ?? ""}</CustomMarkdown>
            ) : null}
            {annotations.length > 0 ? (
                <AnnotationList annotations={annotations} />
            ) : null}
            {logBlock(log)}
            {import.meta.env.DEV ? (
                <DumpJobLog
                    owner={owner}
                    repo={repo}
                    checkRunId={run.checkRunId}
                />
            ) : null}
        </>
    );
}

/** Puts the untouched log and the step list on the clipboard, so the real
 * shape of a job log can be read from a live run. The step stamps and the
 * `##[group]` markers do not line up the way the splitting assumes, and the
 * only way to settle where the boundaries actually are is to look at both
 * sides of the same job side by side. */
function DumpJobLog({
    owner,
    repo,
    checkRunId,
}: {
    owner: string;
    repo: string;
    checkRunId: number | undefined;
}) {
    const { backend } = useAppServices();
    const { copyToClipboard } = useCopyToClipboard();
    const [dumping, setDumping] = useState(false);
    return (
        <div className="flex justify-end">
            <Button
                variant="outline"
                size="sm"
                loading={dumping}
                onClick={async () => {
                    if (checkRunId == null) return;
                    setDumping(true);
                    try {
                        const outcome = await backend.github.getCheckRunLog(
                            owner,
                            repo,
                            checkRunId
                        );
                        if (!outcome.ok) {
                            toastError("Could not read the log", outcome.error);
                            return;
                        }
                        const { raw, steps } = outcome.value;
                        const header = steps
                            .map((step) =>
                                [
                                    `number: ${step.number}`,
                                    `name: ${step.name}`,
                                    `status: ${step.status}`,
                                    `conclusion: ${step.conclusion ?? ""}`,
                                    `started_at: ${step.startedAt ?? ""}`,
                                    `completed_at: ${step.completedAt ?? ""}`,
                                ].join("\t")
                            )
                            .join("\n");
                        const text = [
                            "=== steps (check run / job id " +
                                checkRunId +
                                ") ===",
                            header,
                            "",
                            "=== raw log ===",
                            raw,
                        ].join("\n");
                        copyToClipboard(text);
                        toastManager.add({
                            title: `Copied ${text.length} chars`,
                            type: "success",
                        });
                    } finally {
                        setDumping(false);
                    }
                }}
            >
                Dump job log
            </Button>
        </div>
    );
}

/** The job's steps, each with its own log. Fetched as soon as the dialog
 * opens, since the steps are the reason to open it. */
/** The failure reasons a run reported, as framed blocks so a level and a
 * location read as metadata rather than as loose text. */
function AnnotationList({
    annotations,
}: {
    annotations: GithubCheckAnnotation[];
}) {
    return (
        <div className="flex flex-col gap-2">
            {annotations.map((annotation, index) => (
                <Frame key={index} role="note">
                    <FrameHeader className="flex flex-row items-center gap-1.5 px-2 py-1.5">
                        <CircleAlert
                            className={cn(
                                "size-3.5",
                                annotationLevelClass(annotation.annotationLevel)
                            )}
                        />
                        <span
                            className={cn(
                                "truncate text-xs font-medium",
                                annotationLevelClass(annotation.annotationLevel)
                            )}
                        >
                            {annotation.title || annotation.annotationLevel}
                        </span>
                        <span className="ms-auto truncate font-mono text-xs text-muted-foreground">
                            {annotation.path}
                            {annotation.startLine != null
                                ? `:${annotation.startLine}`
                                : null}
                        </span>
                    </FrameHeader>
                    <FramePanel className="ui-selectable px-3 py-1.5 text-sm whitespace-pre-wrap">
                        {annotation.message}
                    </FramePanel>
                </Frame>
            ))}
        </div>
    );
}

function logBlock(log: ReturnType<typeof useCheckRunLog>) {
    if (log.isError) {
        return (
            <Frame role="note">
                <FrameHeader className="flex flex-row items-center gap-1.5 px-2 py-1.5">
                    <TriangleAlert className="size-3.5 text-destructive" />
                    <span className="text-xs font-medium text-destructive">
                        Log unavailable
                    </span>
                </FrameHeader>
                <FramePanel className="px-2 py-1.5 text-sm">
                    {log.error instanceof Error
                        ? log.error.message
                        : "Could not load the job log."}
                </FramePanel>
            </Frame>
        );
    }
    if (log.data == null) {
        return <JobLogSkeleton />;
    }
    if (log.data.unavailable != null) {
        return (
            <Frame role="note">
                <FrameHeader className="flex flex-row items-center gap-1.5 px-2 py-1.5">
                    <Info className="size-3.5 text-muted-foreground" />
                    <span className="text-xs font-medium text-muted-foreground">
                        No job log
                    </span>
                </FrameHeader>
                <FramePanel className="px-2 py-1.5 text-sm text-muted-foreground">
                    {log.data.unavailable}
                </FramePanel>
            </Frame>
        );
    }
    if (log.data.steps.length === 0) {
        return (
            <Frame role="note">
                <FrameHeader className="flex flex-row items-center gap-1.5 px-2 py-1.5">
                    <Info className="size-3.5 text-muted-foreground" />
                    <span className="text-xs font-medium text-muted-foreground">
                        No log output
                    </span>
                </FrameHeader>
                <FramePanel className="px-2 py-1.5 text-sm text-muted-foreground">
                    This run reported no log output.
                </FramePanel>
            </Frame>
        );
    }
    return <StepList steps={log.data.steps} styles={log.data.styles} />;
}

/** Step-shaped placeholders, so the dialog does not jump when the log
 * lands. */
function JobLogSkeleton() {
    return (
        <div className="flex flex-col gap-2" aria-busy="true">
            {[0, 1, 2, 3].map((index) => (
                <Frame key={index}>
                    <FrameHeader className="flex h-8 flex-row items-center gap-1.5 px-2 py-1">
                        <Skeleton className="size-4 rounded-full" />
                        <Skeleton
                            className={`h-3.5 ${index % 2 === 0 ? "w-2/5" : "w-1/3"}`}
                        />
                    </FrameHeader>
                </Frame>
            ))}
        </div>
    );
}

function StepList({
    steps,
    styles,
}: {
    steps: GithubActionStep[];
    styles: SyntaxStyle[];
}) {
    return (
        <div className="flex flex-col gap-2">
            {steps.map((step) => (
                <StepRow
                    key={step.number}
                    name={step.name}
                    status={step.status}
                    conclusion={step.conclusion}
                    log={step.log}
                    spansByLine={step.spansByLine}
                    styles={styles}
                    open={step.conclusion !== "success"}
                />
            ))}
        </div>
    );
}

/** One step of the job, collapsed by default, opened automatically when the
 * step did not pass so a failure is the first thing shown. */
function StepRow({
    name,
    status,
    conclusion,
    log,
    spansByLine,
    styles,
    open,
}: {
    name: string;
    status: string;
    conclusion: string | undefined;
    log: string;
    spansByLine: number[][];
    styles: SyntaxStyle[];
    open: boolean;
}) {
    const appearance = stepAppearance(status, conclusion);
    return (
        <Collapsible defaultOpen={open}>
            <Frame>
                <FrameHeader className="flex flex-row items-center px-2 py-1">
                    <CollapsibleTrigger className="flex min-w-0 flex-1 items-center gap-1.5 rounded-sm px-0.5 py-0.5 text-sm font-medium [&[data-panel-open]>svg]:rotate-0">
                        <ChevronDown className="size-4 shrink-0 -rotate-90 transition-transform" />
                        <span className={appearance.className}>
                            {appearance.icon}
                        </span>
                        <span className="truncate">{name}</span>
                    </CollapsibleTrigger>
                </FrameHeader>
                <CollapsiblePanel>
                    <FramePanel className="code-hl px-3 py-2 empty:hidden">
                        {log.trim().length > 0 ? (
                            <div className="ui-selectable font-mono text-xs">
                                {log.split("\n").map((line, index) => {
                                    const spans = spansByLine[index] ?? [];
                                    return (
                                        <div
                                            key={index}
                                            className="whitespace-pre-wrap"
                                        >
                                            <HighlightedLine
                                                code={line}
                                                row={
                                                    spans.length > 0
                                                        ? {
                                                              kind: "context",
                                                              content: line,
                                                              spans,
                                                          }
                                                        : undefined
                                                }
                                                styles={styles}
                                            />
                                        </div>
                                    );
                                })}
                            </div>
                        ) : (
                            <p className="text-muted-foreground">No output.</p>
                        )}
                    </FramePanel>
                </CollapsiblePanel>
            </Frame>
        </Collapsible>
    );
}

function stepAppearance(
    status: string,
    conclusion: string | undefined
): { icon: React.ReactNode; className: string } {
    if (status !== "completed") {
        return {
            icon: <CircleDashed className="size-4" />,
            className: "text-info",
        };
    }
    if (conclusion === "success") {
        return {
            icon: <Check className="size-4" />,
            className: "text-success",
        };
    }
    if (conclusion === "skipped") {
        return {
            icon: <CircleSlash className="size-4" />,
            className: "text-muted-foreground",
        };
    }
    return {
        icon: <X className="size-4" />,
        className: "text-destructive",
    };
}

function annotationLevelClass(level: string): string {
    switch (level) {
        case "failure":
        case "error":
            return "text-destructive";
        case "warning":
            return "text-warning";
        default:
            return "text-muted-foreground";
    }
}

function workflowAppearance(run: GithubWorkflowRun): {
    icon: React.ReactNode;
    className: string;
    label: string;
} {
    if (run.status !== "completed") {
        return {
            icon: <CircleDashed className="size-4" />,
            className: "text-info",
            label: "Running",
        };
    }
    if (run.conclusion?.toLowerCase() === "success") {
        return {
            icon: <CheckCheck className="size-4" />,
            className: "text-success",
            label: "Passed",
        };
    }
    return {
        icon: <X className="size-4" />,
        className: "text-destructive",
        label: "Failed",
    };
}
