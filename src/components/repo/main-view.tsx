import Scritto from "@scritto/react";
import { useHotkey } from "@tanstack/react-hotkeys";
import { useEffect, useState } from "react";

import { useTabId } from "@/contexts/tab-context";
import { cn } from "@/lib/utils";
import type { reducerAction, RepoTab, RepoView } from "@/routes/repo-page";

import { CommitDiffView } from "../diff/commit-diff-view";
import { ConflictDiffViewer } from "../diff/conflict-diff-viewer";
import { DiffViewer } from "../diff/diff-viewer";
import { CatCurled, CatSitting, CatStretching } from "../icons/cat";
import { CommitGraphView } from "./history/commit-graph-view";
import { HistoryChartView } from "./history/history-chart-view";
import { IssuesView } from "./issues/issues-view";

function stripChangeSide(changeId: string): string {
    const separator = changeId.indexOf(":");
    return separator === -1 ? changeId : changeId.slice(separator + 1);
}

function getTimeOfDay() {
    const hour = new Date().getHours();
    if (hour >= 5 && hour < 12) return "morning";
    if (hour >= 12 && hour < 22) return "day";
    return "night";
}

const STROKE_WIDTH = 0.3;

function EmptyState({ label }: { label: string }) {
    const period = getTimeOfDay();

    return (
        <div className="flex size-full flex-col items-center justify-center">
            {period === "morning" && (
                <CatStretching className="size-48" strokeWidth={STROKE_WIDTH} />
            )}
            {period === "day" && (
                <CatSitting className="size-48" strokeWidth={STROKE_WIDTH} />
            )}
            {period === "night" && (
                <CatCurled className="size-48" strokeWidth={STROKE_WIDTH} />
            )}
            <span className="relative -mt-3 inline-block font-display text-4xl whitespace-nowrap">
                Select a <Scritto value={label} />
            </span>
        </div>
    );
}

export function MainRepoView({
    repoId,
    tab,
    view,
    generation,
    selectedChangeId,
    selectedCommitId,
    selectedStashId,
    dispatch,
}: {
    repoId: number;
    tab: RepoTab;
    view: RepoView;
    generation?: number;
    selectedChangeId: string | null;
    selectedCommitId: string | null;
    selectedStashId: string | null;
    dispatch: React.ActionDispatch<[action: reducerAction]>;
}) {
    "use no memo";

    const tabId = useTabId();
    const nothingSelected =
        !selectedChangeId && !selectedCommitId && !selectedStashId;
    useHotkey("Escape", () => dispatch({ type: "CLEAR_SELECTION" }), {
        preventDefault: true,
    });

    if (selectedCommitId && view === "graph") {
        return (
            <CommitDiffView
                repoId={repoId}
                commitId={selectedCommitId}
                tabId={tabId}
                dispatch={dispatch}
            />
        );
    }

    if (selectedStashId && tab === "changes") {
        return (
            <CommitDiffView
                repoId={repoId}
                commitId={selectedStashId}
                tabId={tabId}
                dispatch={dispatch}
            />
        );
    }

    if (selectedChangeId && tab === "changes") {
        if (selectedChangeId.startsWith("conflict:")) {
            return (
                <ConflictDiffViewer
                    repoId={repoId}
                    path={selectedChangeId.slice("conflict:".length)}
                    dispatch={dispatch}
                />
            );
        }
        return (
            <DiffViewer
                repoId={repoId}
                paths={
                    selectedChangeId
                        ? [stripChangeSide(selectedChangeId)]
                        : undefined
                }
                generation={generation}
                tabId={tabId}
                dispatch={dispatch}
            />
        );
    }

    if (view === "graph") {
        return (
            <div className="min-h-0 flex-1">
                <CommitGraphView
                    repoId={repoId}
                    selectedId={selectedCommitId}
                    onSelect={(commitId) =>
                        dispatch({ type: "SET_COMMIT", data: commitId })
                    }
                />
            </div>
        );
    }

    if (view === "issues") {
        return (
            <div className="min-h-0 flex-1">
                <IssuesView repoId={repoId} />
            </div>
        );
    }

    return (
        <div className="flex size-full min-h-0 flex-col">
            {tab === "history" && <DeferredHistoryChart repoId={repoId} />}
            {selectedCommitId && tab === "history" && (
                <CommitDiffView
                    repoId={repoId}
                    commitId={selectedCommitId}
                    tabId={tabId}
                    dispatch={dispatch}
                />
            )}
            {nothingSelected && (
                <div
                    className={cn(
                        "min-h-0 flex-1",
                        tab === "history" &&
                            "pb-[calc(var(--spacing)*18.5-1px)]"
                    )}
                >
                    <EmptyState
                        label={tab === "history" ? "Commit" : "Change"}
                    />
                </div>
            )}
        </div>
    );
}

function DeferredHistoryChart({ repoId }: { repoId: number }) {
    const [ready, setReady] = useState(false);
    useEffect(() => {
        if (typeof window === "undefined") return;
        if ("requestIdleCallback" in window) {
            const id = (
                window as Window & {
                    requestIdleCallback: (
                        cb: () => void,
                        opts?: { timeout: number }
                    ) => number;
                    cancelIdleCallback: (id: number) => void;
                }
            ).requestIdleCallback(() => setReady(true), { timeout: 300 });
            return () => window.cancelIdleCallback(id);
        }
        const frame = requestAnimationFrame(() => setReady(true));
        return () => cancelAnimationFrame(frame);
    }, []);
    if (!ready) {
        return (
            <div className="h-[calc(var(--spacing)*18.5-1px)] w-full border-b" />
        );
    }
    return <HistoryChartView repoId={repoId} />;
}
