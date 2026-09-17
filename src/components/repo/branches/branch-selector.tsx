import { useQueryClient } from "@tanstack/react-query";
import {
    ArrowDown,
    ArrowUp,
    Check,
    ChevronDown,
    GitBranch,
    GitCommitVertical,
    GitMerge,
    SearchIcon,
} from "lucide-react";
import * as React from "react";
import { useMemo, useState } from "react";

import { WorktreesManager } from "@/components/repo/worktrees/worktrees-manager";
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
import { Input } from "@/components/ui/input";
import { Popover, PopoverPopup, PopoverTrigger } from "@/components/ui/popover";
import { Separator } from "@/components/ui/separator";
import { Tabs, TabsContent, TabsList, TabsTab } from "@/components/ui/tabs";
import { useMergeActions } from "@/hooks/repositories/use-merge-actions";
import {
    useOperationState,
    useRepoListing,
    useRepositorySnapshot,
    useWorktrees,
} from "@/hooks/repositories/use-repository-queries";
import {
    useCheckoutMutation,
    useCreateBranchMutation,
} from "@/lib/backend/mutations/repository-mutations";
import type { BranchInfo } from "@/lib/backend/protocol";
import { repositoryKeys } from "@/lib/backend/queries/query-keys";
import { BORDER_GRADIENT, REPO_TOOLBAR_TRIGGER_CLASS } from "@/lib/constants";
import { toastError } from "@/lib/toast-error";
import { cn } from "@/lib/utils";

import { AutocompleteInput } from "../../ui/autocomplete";

/** Conservative git refname subset: no whitespace or glob/revision chars. */
function isValidBranchName(name: string): boolean {
    if (
        name.length === 0 ||
        name.startsWith("-") ||
        name.startsWith("/") ||
        name.endsWith(".") ||
        name.endsWith("/") ||
        name.includes("..") ||
        name.includes("//") ||
        name.includes("@{")
    ) {
        return false;
    }
    return /^[^\s~^:?*[\\]+$/.test(name);
}

/** Conventional defaults until the backend exposes the remote HEAD symref. */
const DEFAULT_BRANCH_NAMES = ["main", "master"];

function isDefaultBranch(branch: BranchInfo): boolean {
    return DEFAULT_BRANCH_NAMES.includes(branch.name.toLowerCase());
}

interface SelectorGroup {
    value: string;
    items: { value: string; label: string }[];
}

/**
 * The "Current Branch" picker: searchable grouped branch list with upstream
 * counts, click-to-checkout, an inline create-and-switch flow, plus a
 * Worktrees tab for managing linked checkouts.
 */
export function BranchSelector({ repoId }: { repoId: number }) {
    const snapshot = useRepositorySnapshot(repoId);
    const listing = useRepoListing(repoId);
    const worktrees = useWorktrees(repoId);

    const checkout = useCheckoutMutation(repoId);
    const createBranch = useCreateBranchMutation(repoId);
    const operation = useOperationState(repoId);
    const mergeActions = useMergeActions(repoId);
    const queryClient = useQueryClient();
    const mergeInProgress = operation.data?.kind === "merge";

    const [open, setOpen] = useState(false);
    const [view, setView] = useState<"branches" | "worktrees">("branches");
    const [newBranchName, setNewBranchName] = useState("");

    const head = snapshot.data?.head;
    const branches = useMemo(
        () =>
            [...(listing.data?.branches ?? [])].sort((a, b) =>
                a.name.localeCompare(b.name)
            ),
        [listing.data?.branches]
    );

    // Live linked trees that claim a branch, so direct checkout is blocked:
    // the same branch cannot be worked on in two trees at once.
    const branchOwner = useMemo(() => {
        const owners = new Map<string, string>();
        for (const tree of worktrees.data ?? []) {
            if (tree.isCurrent || tree.isPrunable || !tree.branch) continue;
            owners.set(tree.branch, tree.name);
        }
        return owners;
    }, [worktrees.data]);

    const detachedTarget = head?.state === "detached" ? head.target : null;
    const currentName =
        head?.state === "attached" || head?.state === "unborn"
            ? head.branch
            : null;

    const canCreate =
        isValidBranchName(newBranchName.trim()) &&
        !branches.some((branch) => branch.name === newBranchName.trim());

    const grouped = useMemo<SelectorGroup[]>(() => {
        const toItems = (list: BranchInfo[]) =>
            list.map((branch) => ({ value: branch.name, label: branch.name }));
        const result: SelectorGroup[] = [];
        if (detachedTarget) {
            result.push({
                value: "Detached",
                items: [
                    {
                        value: detachedTarget,
                        label: `Detached at ${detachedTarget.slice(0, 7)}`,
                    },
                ],
            });
        }
        const defaults = branches.filter(isDefaultBranch);
        if (defaults.length > 0) {
            result.push({ value: "Default branch", items: toItems(defaults) });
        }
        const others = branches.filter((branch) => !isDefaultBranch(branch));
        if (others.length > 0) {
            result.push({ value: "Other", items: toItems(others) });
        }
        return result;
    }, [branches, detachedTarget]);

    const close = () => {
        setOpen(false);
        setNewBranchName("");
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
        try {
            await checkout.mutateAsync(branchName);
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

    const createAndSwitch = async () => {
        if (!canCreate) return;
        try {
            await createBranch.mutateAsync({ name: newBranchName.trim() });
            queryClient.removeQueries({
                queryKey: repositoryKeys.listing(repoId),
            });
            close();
        } catch (error) {
            toastError("Could not create branch", error);
        }
    };

    const mergeIntoCurrent = async (branchName: string) => {
        if (branchName === currentName || mergeActions.pending) return;
        const outcome = await mergeActions.mergeBranch(branchName);
        if (outcome) close();
    };

    return (
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
                            <GitBranch className="size-7" strokeWidth="1.5px" />
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
                <Tabs
                    value={view}
                    onValueChange={setView}
                    className="min-h-0 gap-0"
                >
                    <TabsList className="w-full">
                        <TabsTab value="branches">Branches</TabsTab>
                        <TabsTab value="worktrees">Worktrees</TabsTab>
                    </TabsList>
                    <TabsContent
                        value="branches"
                        keepMounted
                        className="flex min-h-0 flex-col pt-1.5"
                    >
                        <Command items={grouped}>
                            <AutocompleteInput
                                autoFocus
                                aria-label="Search branches"
                                placeholder="Search branches..."
                                startAddon={<SearchIcon />}
                            />
                            <Separator className="mt-1.5" />
                            <CommandList className="mb-1.5 not-empty:p-0">
                                {(group) => (
                                    <React.Fragment key={group.value}>
                                        <CommandGroup items={group.items}>
                                            <CommandGroupLabel>
                                                {group.value}
                                            </CommandGroupLabel>
                                            <CommandCollection>
                                                {(item) => {
                                                    const branch =
                                                        item.value ===
                                                        detachedTarget
                                                            ? undefined
                                                            : branches.find(
                                                                  (candidate) =>
                                                                      candidate.name ===
                                                                      item.value
                                                              );
                                                    const canMerge =
                                                        !!branch &&
                                                        branch.name !==
                                                            currentName &&
                                                        !mergeActions.pending &&
                                                        !mergeInProgress &&
                                                        !branchOwner.has(
                                                            branch.name
                                                        );
                                                    return branch ? (
                                                        <ContextMenu
                                                            key={item.value}
                                                        >
                                                            <ContextMenuTrigger className="block">
                                                                <CommandItem
                                                                    value={
                                                                        item.value
                                                                    }
                                                                    disabled={
                                                                        checkout.isPending ||
                                                                        branchOwner.has(
                                                                            branch.name
                                                                        )
                                                                    }
                                                                    aria-selected={
                                                                        (!!currentName &&
                                                                            item.value ===
                                                                                currentName) ||
                                                                        undefined
                                                                    }
                                                                    onClick={() =>
                                                                        void switchTo(
                                                                            branch.name
                                                                        )
                                                                    }
                                                                >
                                                                    <Check
                                                                        className={cn(
                                                                            "size-3.5 shrink-0",
                                                                            currentName ===
                                                                                branch.name
                                                                                ? "opacity-100"
                                                                                : "opacity-0"
                                                                        )}
                                                                    />
                                                                    <span className="min-w-0 flex-1 truncate pl-1">
                                                                        {
                                                                            branch.name
                                                                        }
                                                                    </span>
                                                                    {branchOwner.has(
                                                                        branch.name
                                                                    ) ? (
                                                                        <span className="shrink-0 text-sm text-muted-foreground">
                                                                            in{" "}
                                                                            {branchOwner.get(
                                                                                branch.name
                                                                            )}
                                                                        </span>
                                                                    ) : (
                                                                        <>
                                                                            {branch.upstream &&
                                                                                branch
                                                                                    .upstream
                                                                                    .ahead >
                                                                                    0 && (
                                                                                    <UpstreamCount
                                                                                        icon={
                                                                                            <ArrowUp className="size-3" />
                                                                                        }
                                                                                        count={
                                                                                            branch
                                                                                                .upstream
                                                                                                .ahead
                                                                                        }
                                                                                    />
                                                                                )}
                                                                            {branch.upstream &&
                                                                                branch
                                                                                    .upstream
                                                                                    .behind >
                                                                                    0 && (
                                                                                    <UpstreamCount
                                                                                        icon={
                                                                                            <ArrowDown className="size-3" />
                                                                                        }
                                                                                        count={
                                                                                            branch
                                                                                                .upstream
                                                                                                .behind
                                                                                        }
                                                                                    />
                                                                                )}
                                                                        </>
                                                                    )}
                                                                </CommandItem>
                                                            </ContextMenuTrigger>
                                                            <ContextMenuPopup>
                                                                <ContextMenuItem
                                                                    disabled={
                                                                        !canMerge
                                                                    }
                                                                    onClick={() =>
                                                                        void mergeIntoCurrent(
                                                                            branch.name
                                                                        )
                                                                    }
                                                                >
                                                                    <GitMerge />
                                                                    Merge into
                                                                    current
                                                                </ContextMenuItem>
                                                            </ContextMenuPopup>
                                                        </ContextMenu>
                                                    ) : (
                                                        <CommandItem
                                                            value={item.value}
                                                            disabled
                                                        >
                                                            <GitCommitVertical className="size-3.5 shrink-0" />
                                                            <span className="min-w-0 flex-1 truncate">
                                                                {item.label}
                                                            </span>
                                                        </CommandItem>
                                                    );
                                                }}
                                            </CommandCollection>
                                        </CommandGroup>
                                    </React.Fragment>
                                )}
                            </CommandList>
                            <CommandEmpty>No branches found.</CommandEmpty>
                        </Command>
                        <Separator />
                        <div className="flex items-center gap-1.5 pt-1.5">
                            <Input
                                value={newBranchName}
                                onChange={(e) =>
                                    setNewBranchName(e.target.value)
                                }
                                onKeyDown={(e) => {
                                    if (e.key === "Enter") {
                                        e.preventDefault();
                                        void createAndSwitch();
                                    }
                                }}
                                placeholder="New branch name"
                                aria-label="New branch name"
                            />
                            <Button
                                variant="secondary"
                                disabled={!canCreate}
                                loading={createBranch.isPending}
                                onClick={() => void createAndSwitch()}
                            >
                                Create
                            </Button>
                        </div>
                        {!canCreate && newBranchName.trim().length > 0 && (
                            <p className="pt-1.5 text-xs text-destructive">
                                That name is invalid or already exists locally.
                            </p>
                        )}
                    </TabsContent>
                    <TabsContent
                        value="worktrees"
                        keepMounted
                        className="min-h-0 pt-1.5"
                    >
                        <WorktreesManager repoId={repoId} />
                    </TabsContent>
                </Tabs>
            </PopoverPopup>
        </Popover>
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
