import { useMemo } from "react";

import { DiffBody } from "@/components/diff/diff-body";
import { DiffSectionBar } from "@/components/diff/diff-section-bar";
import { Button } from "@/components/ui/button";
import { Spinner } from "@/components/ui/spinner";
import { useDiffViewMode } from "@/hooks/changes/use-diff-view-mode";
import { useMergeActions } from "@/hooks/repositories/use-merge-actions";
import { useConflictFile } from "@/hooks/repositories/use-repository-queries";
import type { ConflictFile, SectionMeta } from "@/lib/backend/protocol";
import type { SectionRows } from "@/lib/backend/streams/diff-session";
import type { reducerAction } from "@/routes/repo-page";

function sectionMeta(file: ConflictFile): SectionMeta {
    return {
        sectionId: file.stage,
        path: file.path,
        kind: "conflicted",
        binary: file.binary,
        image: false,
        complete: true,
    };
}

function StagePane({
    title,
    action,
    repoId,
    path,
    stage,
    mode,
}: {
    title: string;
    action?: { label: string; ariaLabel: string; onTake: () => void };
    repoId: number;
    path: string;
    stage: number;
    mode: "unified" | "split";
}) {
    const file = useConflictFile(repoId, path, stage);
    const rowsBySection = useMemo<SectionRows>(() => {
        const rows = file.data?.rows ?? [];
        return new Map([[0, [...rows]]]);
    }, [file.data?.rows]);

    return (
        <div className="flex min-h-0 min-w-0 flex-1 flex-col border-r last:border-r-0">
            <div className="flex h-8 shrink-0 items-center gap-1.5 border-b px-2">
                <span className="min-w-0 flex-1 truncate font-mono text-xs text-muted-foreground">
                    {title}
                </span>
                {action && (
                    <Button
                        size="xs"
                        variant="secondary"
                        aria-label={action.ariaLabel}
                        onClick={action.onTake}
                    >
                        {action.label}
                    </Button>
                )}
            </div>
            <div className="flex min-h-0 flex-1 flex-col">
                {file.isPending ? (
                    <span className="flex items-center gap-1.5 p-2 text-xs text-muted-foreground">
                        <Spinner className="size-3.5" />
                        Loading…
                    </span>
                ) : file.isError || !file.data ? (
                    <p className="p-2 text-xs text-muted-foreground">
                        This side deleted the file.
                    </p>
                ) : file.data.rows.length === 0 ? (
                    <p className="p-2 text-xs text-muted-foreground">
                        Empty file on this side.
                    </p>
                ) : (
                    <>
                        {file.data.truncated && (
                            <p className="shrink-0 border-b px-2 py-1 text-xs text-muted-foreground">
                                Showing the first 20,000 lines.
                            </p>
                        )}
                        <DiffBody
                            rowsBySection={rowsBySection}
                            sectionId={stage}
                            rowCount={file.data.rows.length}
                            mode={mode}
                            styles={file.data.styles}
                            section={sectionMeta(file.data)}
                        />
                    </>
                )}
            </div>
        </div>
    );
}

export function ConflictDiffViewer({
    repoId,
    path,
    dispatch,
}: {
    repoId: number;
    path: string;
    dispatch: React.ActionDispatch<[action: reducerAction]>;
}) {
    const actions = useMergeActions(repoId);
    const { mode, setMode } = useDiffViewMode();

    const take = (side: "ours" | "theirs") => () => {
        void actions.resolveConflict(path, side).then((resolved) => {
            if (resolved) dispatch({ type: "CLEAR_SELECTION" });
        });
    };

    return (
        <div className="ui-selectable flex size-full min-h-0 min-w-0 flex-col">
            <DiffSectionBar
                path={path}
                kind="conflicted"
                mode={mode}
                onModeChange={setMode}
                dispatch={dispatch}
            />
            <div className="flex min-h-0 flex-1">
                <StagePane
                    title="Base (:1)"
                    repoId={repoId}
                    path={path}
                    stage={1}
                    mode={mode}
                />
                <StagePane
                    title="Ours (:2)"
                    action={{
                        label: "Take ours",
                        ariaLabel: `Take ours for ${path}`,
                        onTake: take("ours"),
                    }}
                    repoId={repoId}
                    path={path}
                    stage={2}
                    mode={mode}
                />
                <StagePane
                    title="Theirs (:3)"
                    action={{
                        label: "Take theirs",
                        ariaLabel: `Take theirs for ${path}`,
                        onTake: take("theirs"),
                    }}
                    repoId={repoId}
                    path={path}
                    stage={3}
                    mode={mode}
                />
            </div>
        </div>
    );
}
