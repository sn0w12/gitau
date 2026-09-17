import * as React from "react";

import { DelayedSpinner } from "@/components/diff/delayed-spinner";
import { DiffBody } from "@/components/diff/diff-body";
import { DiffSectionBar } from "@/components/diff/diff-section-bar";
import { DiffSessionGate } from "@/components/diff/diff-session-gate";
import { Spinner } from "@/components/ui/spinner";
import { useDiffViewMode } from "@/hooks/changes/use-diff-view-mode";
import { useImageDiffViewMode } from "@/hooks/changes/use-image-diff-view-mode";
import type { DiffSessionState } from "@/lib/backend/streams/diff-session";
import type { reducerAction } from "@/routes/repo-page";

/**
 * Streaming diff viewer with unified and split layouts. One session per
 * mount; `paths` scopes it to specific files, otherwise every changed file
 * arrives as its own navigable section.
 */
export function DiffViewer({
    repoId,
    paths,
    generation,
    tabId,
    dispatch,
}: {
    repoId: number;
    paths?: string[];
    /** Repository generation; a change re-streams the worktree diff. */
    generation?: number;
    tabId?: string;
    dispatch: React.ActionDispatch<[action: reducerAction]>;
}) {
    const pathList = React.useMemo(() => paths ?? [], [paths]);
    const request = React.useMemo(
        () => (pathList.length > 0 ? { paths: [...pathList] } : {}),
        // Keyed by contents, not identity: callers may pass fresh arrays,
        // and the streaming session keys itself on the serialized request.
        [pathList]
    );
    // A generation bump (staged/edited file, commit) re-acquires the session
    // so the worktree diff re-streams instead of lingering on old rows.
    return (
        <DiffSessionGate
            repoId={repoId}
            request={request}
            generation={generation}
            tabId={tabId}
            emptyTitle="No changes"
            emptyDescription="This view has nothing to diff right now."
        >
            {(state, _onRetry, controller) => (
                <DiffContent
                    state={state}
                    controller={controller}
                    dispatch={dispatch}
                />
            )}
        </DiffSessionGate>
    );
}

function DiffContent({
    state,
    controller,
    dispatch,
}: {
    state: DiffSessionState;
    controller: import("@/lib/backend/streams/diff-session").DiffSessionController;
    dispatch: React.ActionDispatch<[action: reducerAction]>;
}) {
    const { mode, setMode } = useDiffViewMode();
    const image = state.imagesBySection.get(state.sections[0]?.sectionId ?? -1);
    const { mode: imageMode, setMode: setImageMode } = useImageDiffViewMode();
    // The working-tree viewer shows one file per view; the caller scopes
    // `paths` so a single-section session renders that file.
    const section = state.sections[0];
    if (!section) return null;
    const layout = state.layoutBySection.get(section.sectionId);
    const rowsBySection = state.rowsBySection.get(section.sectionId);
    const styles = state.stylesBySection.get(section.sectionId);

    return (
        <div className="ui-selectable flex size-full min-h-0 min-w-0 flex-col">
            <DiffSectionBar
                path={section.path}
                oldPath={section.oldPath}
                kind={section.kind}
                mode={mode}
                onModeChange={setMode}
                imageMode={section.image ? imageMode : undefined}
                onImageModeChange={section.image ? setImageMode : undefined}
                dispatch={dispatch}
            />
            {layout ? (
                <DiffBody
                    rowsBySection={rowsBySection}
                    sectionId={section.sectionId}
                    rowCount={layout.rowCount}
                    mode={mode}
                    imageMode={imageMode}
                    section={section}
                    image={image}
                    onLoadImage={(sectionId) => {
                        void controller.ensureImage(sectionId);
                    }}
                    styles={styles}
                />
            ) : (
                <div className="flex min-h-0 flex-1 items-center justify-center">
                    <DelayedSpinner className="size-5" />
                </div>
            )}
            <DiffStatusBar
                status={state.status}
                totalRows={state.totalRows}
                additions={state.additions}
                deletions={state.deletions}
            />
        </div>
    );
}

export function DiffStatusBar({
    status,
    totalRows,
    additions,
    deletions,
}: {
    status: "idle" | "running" | "completed" | "failed" | "cancelled" | "stale";
    totalRows: number | null;
    additions: number;
    deletions: number;
}) {
    return (
        <div className="flex h-7 shrink-0 items-center gap-3 border-t px-2 text-xs text-muted-foreground">
            {status === "running" && <Spinner className="size-3" />}
            <span className="ml-auto tabular-nums">
                {totalRows === null ? "" : `${totalRows} lines`}
            </span>
            <span className="text-success tabular-nums">+{additions}</span>
            <span className="text-destructive tabular-nums">
                {"\u2212"}
                {deletions}
            </span>
        </div>
    );
}
