import { useQueryClient } from "@tanstack/react-query";
import {
    ArrowDown,
    ArrowUp,
    Check,
    ChevronDown,
    FolderGit2,
    GitBranch,
    GitCommitVertical,
    GitMerge,
    Lock,
    LockOpen,
    Plus,
    SearchIcon,
    Trash2,
} from "lucide-react";
import * as React from "react";
import { useMemo, useState } from "react";

import { Button } from "@/components/ui/button";
import {
    Command,
    CommandCollection,
    CommandEmpty,
    CommandGroup,
    CommandGroupLabel,
    CommandItem,
    CommandList,
} from "@/components/ui/command";
import {
    ContextMenu,
    ContextMenuItem,
    ContextMenuPopup,
    ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { Popover, PopoverPopup, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { useConfirm } from "@/contexts/confirm-context";
import { useMergeActions } from "@/hooks/repositories/use-merge-actions";
import { useOpenRepository } from "@/hooks/repositories/use-open-repository";
import { repoDisplayName } from "@/hooks/repositories/use-repo-identity";
import {
    useOperationState,
    useRepoListing,
    useRepositorySnapshot,
    useRepositoryStatus,
    useWorktrees,
} from "@/hooks/repositories/use-repository-queries";
import {
    useCheckoutMutation,
    useLockWorktreeMutation,
    useRemoveWorktreeMutation,
    useUnlockWorktreeMutation,
} from "@/lib/backend/mutations/repository-mutations";
import type {
    BranchInfo,
    CheckoutMode,
    WorktreeInfo,
} from "@/lib/backend/protocol";
import { repositoryKeys } from "@/lib/backend/queries/query-keys";
import { BORDER_GRADIENT, REPO_TOOLBAR_TRIGGER_CLASS } from "@/lib/constants";
import { toastError } from "@/lib/toast-error";
import { cn } from "@/lib/utils";

import { AutocompleteInput } from "../../ui/autocomplete";
import { CreateCheckoutDialog } from "./create-checkout-dialog";

/** Conventional defaults until the backend exposes the remote HEAD symref. */
const DEFAULT_BRANCH_NAMES = ["main", "master"];

// A worktree and a branch can share a name, so worktree values live in their
// own namespace to keep the command's item values unique.
const WORKTREE_VALUE_PREFIX = "worktree\u0000";

function isDefaultBranch(branch: BranchInfo): boolean {
    return DEFAULT_BRANCH_NAMES.includes(branch.name.toLowerCase());
}

function worktreeValue(name: string): string {
    return `${WORKTREE_VALUE_PREFIX}${name}`;
}

function worktreeNameFromValue(value: string): string {
    return value.slice(WORKTREE_VALUE_PREFIX.length);
}

type SelectorItem = {
    kind: "detached" | "branch" | "worktree";
    value: string;
    label: string;
};

interface SelectorGroup {
    value: string;
    items: SelectorItem[];
}

/**
 * The "Current Branch" picker: searchable grouped branch list with upstream
 * counts and click-to-checkout. Linked worktrees get their own group below
 * the branches, as rows that open that worktree in the current tab, and the
 * plus button opens the create branch/worktree dialog.
 */
export function BranchSelector({ repoId }: { repoId: number }) {
    const snapshot = useRepositorySnapshot(repoId);
    const listing = useRepoListing(repoId);
    const worktrees = useWorktrees(repoId);

    const checkout = useCheckoutMutation(repoId);
    const removeWorktree = useRemoveWorktreeMutation(repoId);
    const lockWorktree = useLockWorktreeMutation(repoId);
    const unlockWorktree = useUnlockWorktreeMutation(repoId);
    const operation = useOperationState(repoId);
    const mergeActions = useMergeActions(repoId);
    const queryClient = useQueryClient();
    const openRepo = useOpenRepository();
    const { confirm } = useConfirm();
    const status = useRepositoryStatus(repoId);
    const hasChanges =
        (status.data?.entries.length ?? 0) > 0 ||
        (status.data?.conflicts.length ?? 0) > 0;
    const mergeInProgress = operation.data?.kind === "merge";

    const [open, setOpen] = useState(false);
    const [createOpen, setCreateOpen] = useState(false);

    const head = snapshot.data?.head;
    const branches = useMemo(
        () =>
            [...(listing.data?.branches ?? [])].sort((a, b) =>
                a.name.localeCompare(b.name)
            ),
        [listing.data?.branches]
    );

    // Linked trees that claim a branch. The primary tree only claims one when
    // the session is bound to it, since otherwise its branch is someone else's
    // current branch and must stay listable.
    const branchOwner = useMemo(() => {
        const owners = new Map<string, string>();
        for (const tree of worktrees.data ?? []) {
            if (tree.isPrunable || !tree.branch) continue;
            if (tree.isPrimary && tree.isCurrent) continue;
            owners.set(tree.branch, tree.name);
        }
        return owners;
    }, [worktrees.data]);

    // Linked trees only. The tree this tab is bound to stays listed even when
    // it is one of them, otherwise opening a worktree makes its own row
    // vanish and read as the worktree being replaced by a branch.
    const linkedTrees = useMemo(
        () => (worktrees.data ?? []).filter((tree) => !tree.isPrimary),
        [worktrees.data]
    );

    // Standing inside a linked tree leaves no row pointing back at the repo
    // it came from, so the primary tree is offered as a way out. The backend
    // always describes the main worktree there, never the session's own tree,
    // and it is labelled with the repo's directory name because the backend
    // calls it "main", which would be indistinguishable from the default branch.
    const primaryTree = useMemo(
        () => (worktrees.data ?? []).find((tree) => tree.isPrimary),
        [worktrees.data]
    );
    const insideLinkedTree = linkedTrees.some((tree) => tree.isCurrent);
    const returnRow = insideLinkedTree ? primaryTree : undefined;

    const treeRows = useMemo(
        () =>
            returnRow
                ? [
                      {
                          tree: returnRow,
                          label: repoDisplayName(returnRow.path),
                          isCurrent: returnRow.isCurrent,
                      },
                      ...linkedTrees.map((tree) => ({
                          tree,
                          label: tree.name,
                          isCurrent: tree.isCurrent,
                      })),
                  ]
                : linkedTrees.map((tree) => ({
                      tree,
                      label: tree.name,
                      isCurrent: tree.isCurrent,
                  })),
        [linkedTrees, returnRow]
    );
    const treeRowByName = useMemo(
        () => new Map(treeRows.map((row) => [row.tree.name, row])),
        [treeRows]
    );

    const detachedTarget = head?.state === "detached" ? head.target : null;
    const currentName =
        head?.state === "attached" || head?.state === "unborn"
            ? head.branch
            : null;

    const grouped = useMemo<SelectorGroup[]>(() => {
        const toItems = (list: BranchInfo[]): SelectorItem[] =>
            list.map((branch) => ({
                kind: "branch",
                value: branch.name,
                label: branch.name,
            }));
        const result: SelectorGroup[] = [];
        if (detachedTarget) {
            result.push({
                value: "Detached",
                items: [
                    {
                        kind: "detached",
                        value: detachedTarget,
                        label: `Detached at ${detachedTarget.slice(0, 7)}`,
                    },
                ],
            });
        }
        // Only the "Other" group drops claimed branches. A default branch held
        // by a linked tree stays listed, because hiding the sole default
        // branch would remove the group entirely.
        const defaults = branches.filter(isDefaultBranch);
        if (defaults.length > 0) {
            result.push({ value: "Default branch", items: toItems(defaults) });
        }
        const others = branches.filter(
            (branch) =>
                !isDefaultBranch(branch) && !branchOwner.has(branch.name)
        );
        if (others.length > 0) {
            result.push({ value: "Other", items: toItems(others) });
        }
        if (treeRows.length > 0) {
            result.push({
                value: "Worktrees",
                items: treeRows.map((row) => ({
                    kind: "worktree" as const,
                    value: worktreeValue(row.tree.name),
                    label: row.label,
                })),
            });
        }
        return result;
    }, [branches, branchOwner, detachedTarget, treeRows]);

    const close = () => {
        setOpen(false);
    };

    const switchTo = async (branchName: string) => {
        if (branchOwner.has(branchName)) return;
        if (mergeInProgress) {
            toastError(
                "Could not switch branch",
                new Error("Finish or abort the merge first")
            );
            return;
        }
        let mode: CheckoutMode = "safe";
        if (hasChanges) {
            const result = await confirm({
                title: "Uncommitted changes",
                description:
                    "You have uncommitted changes. Keep them on this branch or take them with you?",
                confirmText: "Take changes",
                cancelText: "Keep changes",
            });
            mode = result.confirmed ? "takeChanges" : "keepChanges";
        }
        try {
            await checkout.mutateAsync({
                target: branchName,
                options: { mode },
            });
            // The listing's ahead/behind data is stale for the new HEAD;
            // drop it so the next open re-derives sync state cleanly.
            queryClient.removeQueries({
                queryKey: repositoryKeys.listing(repoId),
            });
            void queryClient.invalidateQueries({
                queryKey: repositoryKeys.snapshot(repoId),
            });
            close();
        } catch (error) {
            toastError("Could not switch branch", error);
        }
    };

    const openWorktree = async (tree: WorktreeInfo, isCurrent: boolean) => {
        // A prunable tree's checkout directory is gone, so there is no
        // repository left to bind the tab to. The tree this tab already shows
        // needs no rebinding either.
        if (tree.isPrunable || isCurrent) return;
        close();
        await openRepo(tree.path);
    };

    const mergeIntoCurrent = async (branchName: string) => {
        if (branchName === currentName || mergeActions.pending) return;
        const outcome = await mergeActions.mergeBranch(branchName);
        if (outcome) close();
    };

    const toggleWorktreeLock = async (tree: WorktreeInfo) => {
        const locked = tree.lockedBy !== undefined;
        try {
            if (locked) {
                await unlockWorktree.mutateAsync({ name: tree.name });
            } else {
                await lockWorktree.mutateAsync({ name: tree.name });
            }
        } catch (error) {
            toastError(
                locked
                    ? "Could not unlock worktree"
                    : "Could not lock worktree",
                error
            );
        }
    };

    const removeTree = async (tree: WorktreeInfo) => {
        const force = tree.isPrunable || tree.lockedBy !== undefined;
        const result = await confirm({
            title: `Remove worktree ${tree.name}?`,
            description: force
                ? "The checkout directory will be deleted. This cannot be undone."
                : "The tree's admin metadata will be pruned.",
            confirmText: "Remove",
            variant: "destructive",
        });
        if (!result.confirmed) return;
        try {
            await removeWorktree.mutateAsync({ name: tree.name, force });
        } catch (error) {
            toastError("Could not remove worktree", error);
        }
    };

    return (
        <>
            <Popover open={open} onOpenChange={setOpen}>
                <PopoverTrigger
                    render={
                        <div
                            className={cn(
                                REPO_TOOLBAR_TRIGGER_CLASS,
                                BORDER_GRADIENT
                            )}
                        >
                            <div className="flex items-center gap-2.5 pr-2">
                                <GitBranch
                                    className="size-7"
                                    strokeWidth="1.5px"
                                />
                                <div className="flex flex-col">
                                    <span className="hidden text-xs text-muted-foreground lg:block">
                                        Current Branch
                                    </span>
                                    <span className="font-semibold">
                                        {currentName ??
                                            (detachedTarget
                                                ? `Detached at ${detachedTarget.slice(0, 7)}`
                                                : "…")}
                                    </span>
                                </div>
                            </div>
                            <ChevronDown className="size-4" />
                        </div>
                    }
                />
                <PopoverPopup className="min-h-0 w-96" align="center">
                    <div className="flex min-h-0 flex-col">
                        <Command items={grouped}>
                            <div className="flex items-center gap-1.5">
                                <div className="min-w-0 flex-1">
                                    <AutocompleteInput
                                        autoFocus
                                        aria-label="Search branches"
                                        placeholder="Search branches..."
                                        startAddon={<SearchIcon />}
                                    />
                                </div>
                                <Button
                                    variant="outline"
                                    size="icon"
                                    aria-label="New branch or worktree"
                                    onClick={() => {
                                        close();
                                        setCreateOpen(true);
                                    }}
                                >
                                    <Plus />
                                </Button>
                            </div>
                            <Separator className="mt-1.5" />
                            <CommandList className="not-empty:p-0">
                                {(group) => (
                                    <React.Fragment key={group.value}>
                                        <CommandGroup items={group.items}>
                                            <CommandGroupLabel>
                                                {group.value}
                                            </CommandGroupLabel>
                                            <CommandCollection>
                                                {(item) => {
                                                    if (
                                                        item.kind === "worktree"
                                                    ) {
                                                        const row =
                                                            treeRowByName.get(
                                                                worktreeNameFromValue(
                                                                    item.value
                                                                )
                                                            );
                                                        if (!row) return null;
                                                        return (
                                                            <WorktreeRow
                                                                key={item.value}
                                                                tree={row.tree}
                                                                label={
                                                                    row.label
                                                                }
                                                                isCurrent={
                                                                    row.isCurrent
                                                                }
                                                                isPrimary={
                                                                    row.tree
                                                                        .isPrimary
                                                                }
                                                                busy={
                                                                    removeWorktree.isPending ||
                                                                    lockWorktree.isPending ||
                                                                    unlockWorktree.isPending
                                                                }
                                                                onOpen={(
                                                                    tree,
                                                                    isCurrent
                                                                ) =>
                                                                    void openWorktree(
                                                                        tree,
                                                                        isCurrent
                                                                    )
                                                                }
                                                                onToggleLock={(
                                                                    tree
                                                                ) =>
                                                                    void toggleWorktreeLock(
                                                                        tree
                                                                    )
                                                                }
                                                                onRemove={(
                                                                    tree
                                                                ) =>
                                                                    void removeTree(
                                                                        tree
                                                                    )
                                                                }
                                                            />
                                                        );
                                                    }
                                                    const branch =
                                                        item.value ===
                                                        detachedTarget
                                                            ? undefined
                                                            : branches.find(
                                                                  (candidate) =>
                                                                      candidate.name ===
                                                                      item.value
                                                              );
                                                    return (
                                                        <BranchRow
                                                            key={item.value}
                                                            item={item}
                                                            branch={branch}
                                                            currentName={
                                                                currentName
                                                            }
                                                            branchOwner={
                                                                branchOwner
                                                            }
                                                            canMerge={
                                                                !!branch &&
                                                                branch.name !==
                                                                    currentName &&
                                                                !mergeActions.pending &&
                                                                !mergeInProgress &&
                                                                !branchOwner.has(
                                                                    branch.name
                                                                )
                                                            }
                                                            disabled={
                                                                checkout.isPending
                                                            }
                                                            onSwitch={(name) =>
                                                                void switchTo(
                                                                    name
                                                                )
                                                            }
                                                            onMerge={(name) =>
                                                                void mergeIntoCurrent(
                                                                    name
                                                                )
                                                            }
                                                        />
                                                    );
                                                }}
                                            </CommandCollection>
                                        </CommandGroup>
                                    </React.Fragment>
                                )}
                            </CommandList>
                            <CommandEmpty>No branches found.</CommandEmpty>
                        </Command>
                    </div>
                </PopoverPopup>
            </Popover>
            <CreateCheckoutDialog
                repoId={repoId}
                existingBranches={branches.map((branch) => branch.name)}
                open={createOpen}
                onClose={() => setCreateOpen(false)}
            />
        </>
    );
}

function BranchRow({
    item,
    branch,
    currentName,
    branchOwner,
    canMerge,
    disabled,
    onSwitch,
    onMerge,
}: {
    item: SelectorItem;
    branch: BranchInfo | undefined;
    currentName: string | null;
    branchOwner: Map<string, string>;
    canMerge: boolean;
    disabled: boolean;
    onSwitch: (name: string) => void;
    onMerge: (name: string) => void;
}) {
    if (!branch) {
        return (
            <CommandItem value={item.value} disabled>
                <GitCommitVertical className="size-3.5 shrink-0" />
                <span className="min-w-0 flex-1 truncate">{item.label}</span>
            </CommandItem>
        );
    }
    return (
        <ContextMenu>
            <ContextMenuTrigger className="block">
                <CommandItem
                    value={item.value}
                    disabled={disabled || branchOwner.has(branch.name)}
                    aria-selected={
                        (!!currentName && item.value === currentName) ||
                        undefined
                    }
                    onClick={() => onSwitch(branch.name)}
                >
                    <Check
                        className={cn(
                            "size-3.5 shrink-0 transition-opacity duration-200 ease-snappy",
                            currentName === branch.name
                                ? "opacity-100"
                                : "opacity-0"
                        )}
                    />
                    <span className="min-w-0 flex-1 truncate pl-1">
                        {branch.name}
                    </span>
                    {branchOwner.has(branch.name) ? (
                        <span className="shrink-0 text-sm text-muted-foreground">
                            in {branchOwner.get(branch.name)}
                        </span>
                    ) : (
                        <>
                            {branch.upstream && branch.upstream.ahead > 0 && (
                                <UpstreamCount
                                    icon={<ArrowUp className="size-3" />}
                                    count={branch.upstream.ahead}
                                />
                            )}
                            {branch.upstream && branch.upstream.behind > 0 && (
                                <UpstreamCount
                                    icon={<ArrowDown className="size-3" />}
                                    count={branch.upstream.behind}
                                />
                            )}
                        </>
                    )}
                </CommandItem>
            </ContextMenuTrigger>
            <ContextMenuPopup>
                <ContextMenuItem
                    disabled={!canMerge}
                    onClick={() => onMerge(branch.name)}
                >
                    <GitMerge />
                    Merge into current
                </ContextMenuItem>
            </ContextMenuPopup>
        </ContextMenu>
    );
}

function WorktreeRow({
    tree,
    label,
    isCurrent,
    isPrimary,
    busy,
    onOpen,
    onToggleLock,
    onRemove,
}: {
    tree: WorktreeInfo;
    label: string;
    isCurrent: boolean;
    isPrimary: boolean;
    busy: boolean;
    onOpen: (tree: WorktreeInfo, isCurrent: boolean) => void;
    onToggleLock: (tree: WorktreeInfo) => void;
    onRemove: (tree: WorktreeInfo) => void;
}) {
    const locked = tree.lockedBy !== undefined;
    return (
        <ContextMenu>
            <ContextMenuTrigger className="block">
                <CommandItem
                    value={worktreeValue(tree.name)}
                    disabled={tree.isPrunable}
                    aria-selected={isCurrent || undefined}
                    onClick={() => onOpen(tree, isCurrent)}
                >
                    {isCurrent ? (
                        <Check className="size-3.5 shrink-0" />
                    ) : (
                        <FolderGit2 className="size-3.5 shrink-0 opacity-0" />
                    )}
                    <span className="min-w-0 flex-1 truncate pl-1">
                        {label}
                    </span>
                    {tree.isPrunable && (
                        <span className="shrink-0 font-mono text-xs text-destructive">
                            prunable
                        </span>
                    )}
                </CommandItem>
            </ContextMenuTrigger>
            {isPrimary ? null : (
                <ContextMenuPopup>
                    <ContextMenuItem
                        disabled={busy}
                        onClick={() => onToggleLock(tree)}
                    >
                        {locked ? <LockOpen /> : <Lock />}
                        {locked ? "Unlock worktree" : "Lock worktree"}
                    </ContextMenuItem>
                    <ContextMenuItem
                        variant="destructive"
                        disabled={busy}
                        onClick={() => onRemove(tree)}
                    >
                        <Trash2 />
                        {tree.isPrunable ? "Prune worktree" : "Remove worktree"}
                    </ContextMenuItem>
                </ContextMenuPopup>
            )}
        </ContextMenu>
    );
}

function UpstreamCount({
    icon,
    count,
}: {
    icon: React.ReactNode;
    count: number;
}) {
    return (
        <span className="flex shrink-0 items-center gap-0.5 font-mono text-xs text-muted-foreground">
            {icon}
            {count}
        </span>
    );
}
