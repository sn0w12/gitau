import { ChevronDown, FolderOpen, SquarePen } from "lucide-react";
import * as React from "react";
import { type ComponentType } from "react";

import { DiffBody } from "@/components/diff/diff-body";
import {
    DiffSectionBar,
    useClampedIndex,
} from "@/components/diff/diff-section-bar";
import { DiffSessionGate } from "@/components/diff/diff-session-gate";
import { DiffStatusBar } from "@/components/diff/diff-viewer";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useDiffViewMode } from "@/hooks/changes/use-diff-view-mode";
import { useImageDiffViewMode } from "@/hooks/changes/use-image-diff-view-mode";
import { useFixedHeightWindow } from "@/hooks/diff/use-fixed-height-window";
import { useOpenInEditor } from "@/hooks/repositories/use-open-in-editor";
import { useRepoPath } from "@/hooks/repositories/use-repo-path";
import { useRevealInFileManager } from "@/hooks/repositories/use-reveal-in-file-manager";
import { useSetting } from "@/hooks/settings/use-setting";
import type { SectionMeta } from "@/lib/backend/protocol";
import type { DiffSessionState } from "@/lib/backend/streams/diff-session";
import type { RepoStateAction } from "@/lib/routing/repo-search";
import { pluralize } from "@/lib/utils";

import { SplitPath } from "../repo/split-path";
import {
    Collapsible,
    CollapsiblePanel,
    CollapsibleTrigger,
} from "../ui/collapsible";
import {
    ContextMenu,
    ContextMenuItem,
    ContextMenuPopup,
    ContextMenuTrigger,
} from "../ui/context-menu";
import {
    TooltipCreateHandle,
    TooltipPayloadHost,
    TooltipProvider,
    TooltipTrigger,
} from "../ui/tooltip";
import { ChangeIcon } from "./change-icon";

/**
 * All files of one commit with their diffs: a file list fed by the stream's
 * section metadata beside the active file's unified/split diff.
 */
export function CommitDiffView({
    repoId,
    commitId,
    tabId,
    dispatch,
}: {
    repoId: number;
    commitId: string;
    tabId?: string;
    dispatch: React.ActionDispatch<[action: RepoStateAction]>;
}) {
    const request = React.useMemo(
        () => ({ comparison: { commitToParent: { commit: commitId } } }),
        [commitId]
    );
    const repoPath = useRepoPath(repoId);

    return (
        <DiffSessionGate
            repoId={repoId}
            request={request}
            tabId={tabId}
            onEmpty={() => dispatch({ type: "CLEAR_SELECTION" })}
        >
            {(state, _onRetry, controller) => (
                <CommitDiffContent
                    state={state}
                    controller={controller}
                    dispatch={dispatch}
                    repoPath={repoPath}
                />
            )}
        </DiffSessionGate>
    );
}

const handle = TooltipCreateHandle<ComponentType>();

const FILE_ROW_HEIGHT_PX = 30;

function CommitDiffContent({
    state,
    controller,
    dispatch,
    repoPath,
}: {
    state: DiffSessionState;
    controller: import("@/lib/backend/streams/diff-session").DiffSessionController;
    dispatch: React.ActionDispatch<[action: RepoStateAction]>;
    repoPath: string;
}) {
    const { mode, setMode } = useDiffViewMode();
    const { mode: imageMode, setMode: setImageMode } = useImageDiffViewMode();
    const sections = state.sections;
    const resetKey = `${state.sessionId}:${sections.length}`;
    const [index, setIndex] = useClampedIndex(sections.length, resetKey);
    const activeIdx = Math.min(index, sections.length - 1);
    const section = sections[activeIdx];
    const layout = state.layoutBySection.get(section.sectionId);
    const rowsBySection = state.rowsBySection.get(section.sectionId);
    const styles = state.stylesBySection.get(section.sectionId);

    const selectPath = React.useCallback(
        (sectionId: number) => {
            const idx = sections.findIndex((s) => s.sectionId === sectionId);
            if (idx >= 0) setIndex(idx);
        },
        [sections, setIndex]
    );

    return (
        <div
            className="flex size-full min-h-0 min-w-0"
            data-session={state.sessionId}
            data-status={state.status}
            data-paths={state.sections.map((s) => s.path).join("|")}
            data-active-idx={activeIdx}
        >
            <CommitFileList
                sections={sections}
                activeSectionId={section.sectionId}
                repoPath={repoPath}
                onSelect={selectPath}
            />

            <div className="ui-selectable flex min-w-0 flex-1 flex-col">
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
                {layout && (
                    <DiffBody
                        rowsBySection={rowsBySection}
                        sectionId={section.sectionId}
                        rowCount={layout.rowCount}
                        mode={mode}
                        imageMode={imageMode}
                        section={section}
                        image={state.imagesBySection.get(section.sectionId)}
                        onLoadImage={(sectionId) => {
                            void controller.ensureImage(sectionId);
                        }}
                        expansions={state.expansionsBySection.get(
                            section.sectionId
                        )}
                        oldLineCount={layout.oldLineCount}
                        canExpand={state.status === "completed"}
                        onExpand={(gapKey, direction, window) => {
                            void controller.expandContext(
                                section.sectionId,
                                gapKey,
                                direction,
                                window
                            );
                        }}
                        styles={styles}
                    />
                )}
                <DiffStatusBar
                    status={state.status}
                    totalRows={state.totalRows}
                    additions={state.additions}
                    deletions={state.deletions}
                />
            </div>
        </div>
    );
}

function CommitFileList({
    sections,
    activeSectionId,
    repoPath,
    onSelect,
}: {
    sections: SectionMeta[];
    activeSectionId: number;
    repoPath: string;
    onSelect: (sectionId: number) => void;
}) {
    const { value: open, setValue: setOpen } = useSetting("commitFileListOpen");
    const { enabled, openInEditor } = useOpenInEditor();
    const revealInFileManager = useRevealInFileManager();

    const listRef = React.useRef<HTMLDivElement>(null);
    const { start, end, totalHeight, offsetTop } = useFixedHeightWindow({
        count: sections.length,
        rowHeight: FILE_ROW_HEIGHT_PX,
        containerRef: listRef,
    });

    return (
        <Collapsible
            className="group flex min-h-0 flex-col border-r"
            open={open}
            onOpenChange={setOpen}
        >
            <CollapsibleTrigger className="flex size-8 w-full shrink-0 touch-manipulation items-center justify-start gap-1 border-b px-2 text-sm group-data-closed:justify-center">
                <ChevronDown className="size-4.5 group-data-closed:-rotate-90" />
                <span className="text-xs group-data-closed:hidden">
                    {sections.length}{" "}
                    {pluralize("Changed File", sections.length)}
                </span>
            </CollapsibleTrigger>
            <CollapsiblePanel
                hidden={false}
                keepMounted
                className="min-h-0 w-[calc(var(--spacing)*64-1px)] flex-1 transition-none data-closed:w-8.5"
            >
                <ScrollArea
                    scrollFade
                    scrollX={false}
                    scrollBar={open}
                    viewportRef={listRef}
                >
                    <TooltipProvider delay={open ? 300 : 50}>
                        <ul
                            data-slot="commit-file-list"
                            className="relative min-w-full"
                            style={{
                                height: totalHeight,
                                paddingTop: offsetTop,
                            }}
                        >
                            {sections.slice(start, end).map((s) => (
                                <CommitFileRow
                                    key={s.sectionId}
                                    section={s}
                                    selected={s.sectionId === activeSectionId}
                                    repoPath={repoPath}
                                    enabled={enabled}
                                    onSelect={onSelect}
                                    onOpenInEditor={openInEditor}
                                    onRevealInFileManager={revealInFileManager}
                                />
                            ))}
                        </ul>
                        <TooltipPayloadHost handle={handle} side="right" />
                    </TooltipProvider>
                </ScrollArea>
            </CollapsiblePanel>
        </Collapsible>
    );
}

const CommitFileRow = React.memo(function CommitFileRow({
    section,
    selected,
    repoPath,
    enabled,
    onSelect,
    onOpenInEditor,
    onRevealInFileManager,
}: {
    section: SectionMeta;
    selected: boolean;
    repoPath: string;
    enabled: boolean;
    onSelect: (sectionId: number) => void;
    onOpenInEditor: (path: string, relativePath?: string) => Promise<void>;
    onRevealInFileManager: (
        path: string,
        relativePath?: string
    ) => Promise<void>;
}) {
    return (
        <TooltipTrigger
            handle={handle}
            payload={() => <SplitPath path={section.path} />}
            render={
                <li className="max-w-64" style={{ height: FILE_ROW_HEIGHT_PX }}>
                    <ContextMenu>
                        <ContextMenuTrigger
                            render={
                                <button
                                    type="button"
                                    aria-selected={selected}
                                    data-selected={selected || undefined}
                                    onClick={() => onSelect(section.sectionId)}
                                    className="flex h-full w-full items-center gap-1.5 px-2 py-1.5 text-left hover:bg-accent data-selected:bg-accent/64"
                                >
                                    <ChangeIcon
                                        change={section.kind}
                                        className="size-4.5 min-w-4.5"
                                    />
                                    <SplitPath
                                        path={section.path}
                                        className="text-xs group-data-closed:hidden"
                                    />
                                </button>
                            }
                        />
                        {repoPath && (
                            <ContextMenuPopup>
                                <ContextMenuItem
                                    disabled={!enabled}
                                    onClick={() =>
                                        void onOpenInEditor(
                                            repoPath,
                                            section.path
                                        )
                                    }
                                >
                                    <SquarePen />
                                    Open in editor
                                </ContextMenuItem>
                                <ContextMenuItem
                                    onClick={() =>
                                        void onRevealInFileManager(
                                            repoPath,
                                            section.path
                                        )
                                    }
                                >
                                    <FolderOpen />
                                    Reveal in file manager
                                </ContextMenuItem>
                            </ContextMenuPopup>
                        )}
                    </ContextMenu>
                </li>
            }
        />
    );
});
