import {
    Archive,
    ChevronUp,
    FolderOpen,
    Minus,
    Plus,
    SquarePen,
    Undo2,
} from "lucide-react";
import {
    AnimatePresence,
    motion,
    useReducedMotion,
    type Variants,
} from "motion/react";
import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode, RefObject } from "react";

import { ChangeIcon } from "@/components/diff/change-icon";
import { CommitForm } from "@/components/repo/changes/commit-form";
import { StashChangesDialog } from "@/components/repo/changes/stash-changes-dialog";
import { StashedChanges } from "@/components/repo/changes/stashed-changes";
import { SplitPath } from "@/components/repo/split-path";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
    Collapsible,
    CollapsibleContent,
    CollapsibleTrigger,
} from "@/components/ui/collapsible";
import {
    ContextMenu,
    ContextMenuItem,
    ContextMenuPopup,
    ContextMenuSeparator,
    ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Input } from "@/components/ui/input";
import { useChangeActions } from "@/hooks/repositories/use-change-actions";
import { useOpenInEditor } from "@/hooks/repositories/use-open-in-editor";
import { useRepoPath } from "@/hooks/repositories/use-repo-path";
import { useRevealInFileManager } from "@/hooks/repositories/use-reveal-in-file-manager";
import type { StatusEntry } from "@/lib/backend/protocol";
import { EASE_SNAPPY } from "@/lib/motion";

import { Badge } from "../../ui/badge";
import { ScrollArea } from "../../ui/scroll-area";

function changePath(changeId: string): string {
    const separator = changeId.indexOf(":");
    return separator === -1 ? changeId : changeId.slice(separator + 1);
}

// Conflicted files open in the 3-way viewer; everything else selects its
// staged/unstaged row id for the worktree diff.
function selectIdFor(entry: StatusEntry): string {
    return entry.kind === "conflicted" ? `conflict:${entry.path}` : entry.id;
}

function stashRequestFor(targets: StatusEntry[]) {
    return {
        paths: [...new Set(targets.map((entry) => entry.path))],
        defaultIncludeUntracked: targets.some(
            (entry) => entry.kind === "untracked"
        ),
    };
}

function stashTargetsFor(
    checked: StatusEntry[],
    entry: StatusEntry
): StatusEntry[] {
    return checked.length > 0 ? checked : [entry];
}

function stashLabelFor(checkedCount: number): string {
    return checkedCount > 1
        ? `Stash ${checkedCount} selected…`
        : "Stash this change";
}

function stashHeaderLabel(checkedCount: number): string {
    return checkedCount > 0
        ? `Stash ${checkedCount} selected`
        : "Stash all changes";
}

function runStashFor(
    checked: StatusEntry[],
    openFor: (targets: StatusEntry[]) => void,
    openAll: () => void
): void {
    if (checked.length > 0) {
        openFor(checked);
    } else {
        openAll();
    }
}

export function ChangesPanel({
    repoId,
    branch,
    selectedId,
    onSelectedChange,
    selectedStashId,
    onSelectedStash,
}: {
    repoId: number;
    branch: string;
    selectedId: string | null;
    onSelectedChange: (id: string | null) => void;
    selectedStashId: string | null;
    onSelectedStash: (id: string | null) => void;
}) {
    const {
        status,
        stage,
        unstage,
        discard,
        stageEntries,
        unstageEntries,
        discardEntries,
    } = useChangeActions(repoId);
    const repoPath = useRepoPath(repoId);
    const entries = useMemo(
        () => status.data?.entries ?? [],
        [status.data?.entries]
    );

    // Stage or unstage rewrites the row id's side prefix (`worktree:p` to
    // `index:p`), so the exact id stops matching and the moved row loses its
    // highlight. Fall back to matching the path once the selected id leaves
    // the report, which keeps a file selected on both sides distinct.
    const selectedPath = selectedId ? changePath(selectedId) : null;
    const selectedIdPresent =
        selectedId !== null &&
        entries.some(
            (entry) =>
                entry.id === selectedId || selectIdFor(entry) === selectedId
        );
    const isSelected = (entry: StatusEntry) =>
        selectedId === entry.id ||
        selectedId === selectIdFor(entry) ||
        (selectedPath !== null &&
            !selectedIdPresent &&
            entry.path === selectedPath);

    // Row callbacks must keep identity across renders or the memoized rows
    // re-render on every selection; refs carry the values that change and
    // are refreshed after each render so an event handler reads the latest.
    const selectedIdRef = useRef(selectedId);
    const onSelectedChangeRef = useRef(onSelectedChange);
    const selectChange = useCallback((entry: StatusEntry) => {
        const id = selectIdFor(entry);
        onSelectedChangeRef.current(selectedIdRef.current === id ? null : id);
    }, []);

    // A ref, not state: motion callbacks resolve when an exit or enter
    // animation starts, which is after the render that removed the row.
    // Reset on every status refetch so unrelated invalidation never
    // animates list changes.
    const animateOpRef = useRef(false);
    useEffect(() => {
        animateOpRef.current = false;
    }, [status.data]);

    const [filter, setFilter] = useState("");
    const [stashRequest, setStashRequest] = useState<{
        paths?: string[];
        defaultIncludeUntracked: boolean;
    } | null>(null);
    const [stagedOpen, setStagedOpen] = useState(true);
    const [changesOpen, setChangesOpen] = useState(true);
    const normalizedFilter = filter.trim().toLowerCase();
    const matches = (entry: StatusEntry) =>
        !normalizedFilter ||
        entry.path.toLowerCase().includes(normalizedFilter) ||
        (entry.oldPath?.toLowerCase().includes(normalizedFilter) ?? false);

    const stagedEntries = entries.filter(
        (entry) => entry.side === "index" && matches(entry)
    );
    const unstagedEntries = entries.filter(
        (entry) => entry.side === "worktree" && matches(entry)
    );

    // Row checkboxes; ids are side-prefixed, so an entry moving between
    // sections (staging) no longer matches the checked id. A mutation that
    // acts on a checked entry unchecks it, so a path that leaves the report
    // and later returns never comes back pre-checked.
    const [checkedIds, setCheckedIds] = useState<ReadonlySet<string>>(
        () => new Set()
    );
    const checkedStaged = stagedEntries.filter((entry) =>
        checkedIds.has(entry.id)
    );
    const checkedUnstaged = unstagedEntries.filter((entry) =>
        checkedIds.has(entry.id)
    );
    const checkedEntries = entries.filter((entry) => checkedIds.has(entry.id));

    const openStashAll = useCallback(
        () => setStashRequest({ defaultIncludeUntracked: false }),
        []
    );
    const openStashFor = useCallback(
        (targets: StatusEntry[]) => setStashRequest(stashRequestFor(targets)),
        []
    );
    const stashLabel = stashLabelFor(checkedEntries.length);
    const stashProps: { paths?: string[]; defaultIncludeUntracked: boolean } =
        stashRequest ?? { defaultIncludeUntracked: false };

    const toggleChecked = useCallback((id: string) => {
        setCheckedIds((prev) => {
            const next = new Set(prev);
            if (next.has(id)) {
                next.delete(id);
            } else {
                next.add(id);
            }
            return next;
        });
    }, []);

    const setSectionChecked = (ids: string[], checked: boolean) => {
        setCheckedIds((prev) => {
            const next = new Set(prev);
            for (const id of ids) {
                if (checked) {
                    next.add(id);
                } else {
                    next.delete(id);
                }
            }
            return next;
        });
    };

    const uncheck = useCallback((ids: Iterable<string>) => {
        setCheckedIds((prev) => {
            const next = new Set(prev);
            for (const id of ids) {
                next.delete(id);
            }
            return next;
        });
    }, []);

    const stageEntriesRef = useRef(stageEntries);
    const unstageEntriesRef = useRef(unstageEntries);
    const discardEntriesRef = useRef(discardEntries);
    const checkedEntriesRef = useRef(checkedEntries);

    // Latest-value refs for the memoized row callbacks; refreshed after each
    // render so a click reads what the last render showed.
    useEffect(() => {
        selectedIdRef.current = selectedId;
        onSelectedChangeRef.current = onSelectedChange;
        stageEntriesRef.current = stageEntries;
        unstageEntriesRef.current = unstageEntries;
        discardEntriesRef.current = discardEntries;
        checkedEntriesRef.current = checkedEntries;
    });

    const stageOne = useCallback(
        (entry: StatusEntry) => {
            animateOpRef.current = true;
            uncheck([entry.id]);
            void stageEntriesRef.current([entry]);
        },
        [uncheck]
    );
    const unstageOne = useCallback(
        (entry: StatusEntry) => {
            animateOpRef.current = true;
            uncheck([entry.id]);
            void unstageEntriesRef.current([entry]);
        },
        [uncheck]
    );
    const discardOne = useCallback(
        (entry: StatusEntry) => {
            animateOpRef.current = true;
            uncheck([entry.id]);
            void discardEntriesRef.current([entry]);
        },
        [uncheck]
    );
    const stashOne = useCallback(
        (entry: StatusEntry) =>
            openStashFor(stashTargetsFor(checkedEntriesRef.current, entry)),
        [openStashFor]
    );

    const stagedIds = stagedEntries.map((entry) => entry.id);
    const unstagedIds = unstagedEntries.map((entry) => entry.id);

    // Commit applies every staged change; a selection whose path was staged
    // is committed and must not linger as a stale diff view. The selection id
    // can point at an already-stale side prefix (e.g. a staged row selected
    // while unstaged), so match on the path, not the id.
    const handleCommitted = () => {
        if (!selectedId) return;
        const committedPaths = new Set(
            stagedEntries.map((entry) => entry.path)
        );
        if (committedPaths.has(changePath(selectedId))) onSelectedChange(null);
    };

    // Header actions target the checked entries when any are checked,
    // falling back to the whole section otherwise.
    const stageTargets =
        checkedUnstaged.length > 0 ? checkedUnstaged : unstagedEntries;
    const unstageTargets =
        checkedStaged.length > 0 ? checkedStaged : stagedEntries;
    const discardTargets =
        checkedUnstaged.length > 0 ? checkedUnstaged : unstagedEntries;

    function isOpen(pending: boolean, items: number, open: boolean) {
        return !pending && items > 0 && open;
    }

    return (
        <div className="flex h-full min-h-0 flex-col">
            <div className="border-b px-0.5 pb-0.5">
                <Input
                    id="changes-filter-input"
                    value={filter}
                    onChange={(e) => {
                        animateOpRef.current = false;
                        setFilter(e.target.value);
                    }}
                    placeholder="Filter files..."
                    aria-label="Filter files"
                />
            </div>
            <ScrollArea scrollFade className="min-h-0 flex-1">
                <Collapsible
                    open={isOpen(
                        status.isPending,
                        stagedEntries.length,
                        stagedOpen
                    )}
                    onOpenChange={setStagedOpen}
                >
                    <SectionHeader
                        title="Staged"
                        count={stagedEntries.length}
                        disabled={stagedEntries.length === 0}
                        checked={
                            stagedEntries.length > 0 &&
                            checkedStaged.length === stagedEntries.length
                        }
                        indeterminate={
                            checkedStaged.length > 0 &&
                            checkedStaged.length < stagedEntries.length
                        }
                        onCheckedChange={(checked) =>
                            setSectionChecked(stagedIds, checked)
                        }
                    >
                        <Button
                            size="icon-2xs"
                            aria-label={
                                checkedStaged.length > 0
                                    ? `Unstage ${checkedStaged.length} selected`
                                    : "Unstage all"
                            }
                            disabled={
                                unstage.isPending || stagedEntries.length === 0
                            }
                            onClick={() => {
                                animateOpRef.current =
                                    unstageTargets.length <= 1;
                                if (checkedStaged.length > 0) {
                                    uncheck(
                                        checkedStaged.map((entry) => entry.id)
                                    );
                                }
                                void unstageEntries(unstageTargets);
                            }}
                        >
                            <Minus />
                        </Button>
                    </SectionHeader>
                    <CollapsibleContent>
                        <AnimatePresence initial={false}>
                            {stagedEntries.map((entry) => (
                                <ChangeRow
                                    key={entry.id}
                                    entry={entry}
                                    checked={checkedIds.has(entry.id)}
                                    side="index"
                                    onCheckedChange={toggleChecked}
                                    selected={isSelected(entry)}
                                    onSelect={selectChange}
                                    busy={
                                        stage.isPending ||
                                        unstage.isPending ||
                                        discard.isPending
                                    }
                                    onStage={stageOne}
                                    onUnstage={unstageOne}
                                    onDiscard={discardOne}
                                    onStash={stashOne}
                                    stashLabel={stashLabel}
                                    repoPath={repoPath}
                                    animateOpRef={animateOpRef}
                                />
                            ))}
                        </AnimatePresence>
                    </CollapsibleContent>
                </Collapsible>
                <Collapsible
                    open={isOpen(
                        status.isPending,
                        unstagedEntries.length,
                        changesOpen
                    )}
                    onOpenChange={setChangesOpen}
                    className="border-t"
                >
                    <SectionHeader
                        title="Unstaged"
                        count={unstagedEntries.length}
                        disabled={unstagedEntries.length === 0}
                        checked={
                            unstagedEntries.length > 0 &&
                            checkedUnstaged.length === unstagedEntries.length
                        }
                        indeterminate={
                            checkedUnstaged.length > 0 &&
                            checkedUnstaged.length < unstagedEntries.length
                        }
                        onCheckedChange={(checked) =>
                            setSectionChecked(unstagedIds, checked)
                        }
                    >
                        <Button
                            size="icon-2xs"
                            variant="default"
                            aria-label={stashHeaderLabel(checkedEntries.length)}
                            disabled={entries.length === 0}
                            onClick={() =>
                                runStashFor(
                                    checkedEntries,
                                    openStashFor,
                                    openStashAll
                                )
                            }
                        >
                            <Archive />
                        </Button>
                        <Button
                            size="icon-2xs"
                            variant="destructive"
                            aria-label={
                                checkedUnstaged.length > 0
                                    ? `Discard ${checkedUnstaged.length} selected`
                                    : "Discard all changes"
                            }
                            disabled={
                                discard.isPending ||
                                unstagedEntries.length === 0
                            }
                            onClick={() => {
                                animateOpRef.current =
                                    discardTargets.length <= 1;
                                if (checkedUnstaged.length > 0) {
                                    uncheck(
                                        checkedUnstaged.map((entry) => entry.id)
                                    );
                                }
                                void discardEntries(discardTargets);
                            }}
                        >
                            <Undo2 />
                        </Button>
                        <Button
                            size="icon-2xs"
                            variant="info"
                            aria-label={
                                checkedUnstaged.length > 0
                                    ? `Stage ${checkedUnstaged.length} selected`
                                    : "Stage all changes"
                            }
                            disabled={
                                stage.isPending || unstagedEntries.length === 0
                            }
                            onClick={() => {
                                animateOpRef.current = stageTargets.length <= 1;
                                if (checkedUnstaged.length > 0) {
                                    uncheck(
                                        checkedUnstaged.map((entry) => entry.id)
                                    );
                                }
                                void stageEntries(stageTargets);
                            }}
                        >
                            <Plus />
                        </Button>
                    </SectionHeader>
                    <CollapsibleContent className="flex flex-col">
                        <AnimatePresence initial={false}>
                            {unstagedEntries.map((entry) => (
                                <ChangeRow
                                    key={entry.id}
                                    entry={entry}
                                    checked={checkedIds.has(entry.id)}
                                    side="worktree"
                                    onCheckedChange={toggleChecked}
                                    selected={isSelected(entry)}
                                    onSelect={selectChange}
                                    busy={
                                        stage.isPending ||
                                        unstage.isPending ||
                                        discard.isPending
                                    }
                                    onStage={stageOne}
                                    onUnstage={unstageOne}
                                    onDiscard={discardOne}
                                    onStash={stashOne}
                                    stashLabel={stashLabel}
                                    repoPath={repoPath}
                                    animateOpRef={animateOpRef}
                                />
                            ))}
                        </AnimatePresence>
                    </CollapsibleContent>
                </Collapsible>
            </ScrollArea>
            <StashedChanges
                repoId={repoId}
                selectedId={selectedStashId}
                onSelect={onSelectedStash}
            />
            <div className="border-t px-0.5 pt-1 pb-0.5">
                <CommitForm
                    repoId={repoId}
                    branch={branch}
                    stagedCount={
                        entries.filter((entry) => entry.side === "index").length
                    }
                    onCommitted={handleCommitted}
                />
            </div>
            <StashChangesDialog
                repoId={repoId}
                open={stashRequest !== null}
                paths={stashProps.paths}
                defaultIncludeUntracked={stashProps.defaultIncludeUntracked}
                onClose={() => setStashRequest(null)}
            />
        </div>
    );
}

function SectionHeader({
    title,
    count,
    disabled,
    checked,
    indeterminate,
    onCheckedChange,
    children,
}: {
    title: string;
    count: number;
    disabled: boolean;
    checked: boolean;
    indeterminate: boolean;
    onCheckedChange: (checked: boolean) => void;
    children?: ReactNode;
}) {
    return (
        <div
            className="flex items-center justify-between pe-1 hover:bg-accent data-[disabled=true]:pointer-events-none data-[disabled=true]:opacity-64"
            data-disabled={disabled}
        >
            <CollapsibleTrigger
                className="flex min-w-0 flex-1 items-center gap-1 border-transparent px-1.5 py-0.5 text-foreground data-panel-open:[&_svg]:rotate-180"
                disabled={disabled}
            >
                <ChevronUp className="mr-0.5 size-4 shrink-0 text-muted-foreground" />
                <Checkbox
                    checked={checked}
                    indeterminate={indeterminate}
                    disabled={disabled}
                    onCheckedChange={onCheckedChange}
                    onClick={(e) => e.stopPropagation()}
                    aria-label={`Select all ${title}`}
                    className="[&_svg]:rotate-0!"
                />
                <span className="font-semibold">{title}</span>
                <Badge
                    className="h-5 rounded-md font-mono sm:h-5"
                    variant="outline"
                >
                    {count}
                </Badge>
            </CollapsibleTrigger>
            <div className="flex shrink-0 items-center gap-0.5 pr-2.5">
                {children}
            </div>
        </div>
    );
}

const ChangeRow = memo(function ChangeRow({
    entry,
    checked,
    onCheckedChange,
    selected,
    onSelect,
    busy,
    side,
    onStage,
    onUnstage,
    onDiscard,
    onStash,
    stashLabel,
    repoPath,
    animateOpRef,
}: {
    entry: StatusEntry;
    checked: boolean;
    onCheckedChange: (id: string) => void;
    selected: boolean;
    onSelect: (entry: StatusEntry) => void;
    busy: boolean;
    side: "index" | "worktree";
    onStage: (entry: StatusEntry) => void;
    onUnstage: (entry: StatusEntry) => void;
    onDiscard: (entry: StatusEntry) => void;
    onStash: (entry: StatusEntry) => void;
    stashLabel: string;
    repoPath: string;
    animateOpRef: RefObject<boolean>;
}) {
    const reducedMotion = useReducedMotion();
    const { enabled, openInEditor } = useOpenInEditor();
    const revealInFileManager = useRevealInFileManager();
    const [hovered, setHovered] = useState(false);
    const row = (
        <div
            role="option"
            aria-selected={selected}
            data-selected={selected || undefined}
            onClick={() => onSelect(entry)}
            onPointerEnter={() => setHovered(true)}
            onPointerLeave={() => setHovered(false)}
            className="relative flex w-full cursor-pointer items-center justify-between px-1 py-0.5 hover:bg-accent data-selected:bg-accent/64"
        >
            <span
                aria-hidden="true"
                data-slot="selected-change-indicator"
                className="pointer-events-none absolute top-1/2 left-0 -translate-y-1/2"
            >
                <motion.span
                    className="block w-0.5 rounded-r-full bg-primary"
                    initial={false}
                    animate={{
                        height: selected ? 10 : hovered ? 4 : 0,
                        opacity: selected || hovered ? 1 : 0,
                    }}
                    transition={{ duration: 0.1, ease: EASE_SNAPPY }}
                />
            </span>
            <div className="flex min-w-0 items-center gap-1">
                <ChangeIcon change={entry.kind} className="shrink-0" />
                <Checkbox
                    checked={checked}
                    onCheckedChange={() => onCheckedChange(entry.id)}
                    onClick={(e) => e.stopPropagation()}
                    aria-label={`Select ${entry.path}`}
                />
                <div className="min-w-0">
                    <SplitPath path={entry.path} />
                </div>
            </div>
            <div className="flex gap-0.5 pr-2.5 pl-1">
                {side === "worktree" ? (
                    <>
                        <Button
                            size="icon-2xs"
                            variant="destructive"
                            aria-label={`Discard changes in ${entry.path}`}
                            disabled={busy}
                            onClick={(e) => {
                                e.stopPropagation();
                                onDiscard(entry);
                            }}
                        >
                            <Undo2 />
                        </Button>
                        <Button
                            size="icon-2xs"
                            variant="info"
                            aria-label={`Stage ${entry.path}`}
                            disabled={busy}
                            onClick={(e) => {
                                e.stopPropagation();
                                onStage(entry);
                            }}
                        >
                            <Plus />
                        </Button>
                    </>
                ) : (
                    <Button
                        size="icon-2xs"
                        variant="default"
                        aria-label={`Unstage ${entry.path}`}
                        disabled={busy}
                        onClick={(e) => {
                            e.stopPropagation();
                            onUnstage(entry);
                        }}
                    >
                        <Minus />
                    </Button>
                )}
            </div>
        </div>
    );

    return (
        <motion.div
            initial="enter"
            animate="visible"
            exit="exit"
            variants={rowVariants(animateOpRef, reducedMotion)}
        >
            <ContextMenu>
                <ContextMenuTrigger className="block">{row}</ContextMenuTrigger>
                <ContextMenuPopup>
                    {side === "worktree" ? (
                        <>
                            <ContextMenuItem
                                disabled={busy}
                                onClick={() => onStage(entry)}
                            >
                                <Plus />
                                Stage
                            </ContextMenuItem>
                            <ContextMenuItem
                                disabled={busy}
                                variant="destructive"
                                onClick={() => onDiscard(entry)}
                            >
                                <Undo2 />
                                Discard changes
                            </ContextMenuItem>
                        </>
                    ) : (
                        <ContextMenuItem
                            disabled={busy}
                            onClick={() => onUnstage(entry)}
                        >
                            <Minus />
                            Unstage
                        </ContextMenuItem>
                    )}
                    <ContextMenuItem
                        disabled={busy}
                        onClick={() => onStash(entry)}
                        data-testid="change-menu-stash"
                    >
                        <Archive />
                        {stashLabel}
                    </ContextMenuItem>
                    {repoPath && (
                        <>
                            <ContextMenuSeparator />
                            <ContextMenuItem
                                disabled={!enabled}
                                onClick={() =>
                                    void openInEditor(repoPath, entry.path)
                                }
                                data-testid="change-menu-open-in-editor"
                            >
                                <SquarePen />
                                Open in editor
                            </ContextMenuItem>
                            <ContextMenuItem
                                onClick={() =>
                                    void revealInFileManager(
                                        repoPath,
                                        entry.path
                                    )
                                }
                                data-testid="change-menu-reveal-in-file-manager"
                            >
                                <FolderOpen />
                                Reveal in file manager
                            </ContextMenuItem>
                        </>
                    )}
                </ContextMenuPopup>
            </ContextMenu>
        </motion.div>
    );
});

function rowVariants(
    animateOpRef: RefObject<boolean>,
    reducedMotion: boolean | null
): Variants {
    const noFade = reducedMotion ? { duration: 0 } : undefined;
    return {
        enter: () => (animateOpRef.current ? { opacity: 0 } : { opacity: 1 }),
        visible: {
            opacity: 1,
            transition: noFade ?? { duration: 0.15, ease: "easeOut" },
        },
        exit: () =>
            animateOpRef.current
                ? {
                      opacity: 0,
                      height: 0,
                      overflow: "hidden",
                      transition: {
                          opacity: noFade ?? { duration: 0.12, ease: "easeIn" },
                          height: { duration: 0.15, ease: "easeInOut" },
                      },
                  }
                : {
                      opacity: 1,
                      height: "auto",
                      transition: { duration: 0 },
                  },
    };
}
