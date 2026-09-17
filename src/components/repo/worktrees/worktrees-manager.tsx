import { Lock, LockOpen, Trash2 } from "lucide-react";
import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import {
    Tooltip,
    TooltipContent,
    TooltipTrigger,
} from "@/components/ui/tooltip";
import { useConfirm } from "@/contexts/confirm-context";
import { useWorktrees } from "@/hooks/repositories/use-repository-queries";
import {
    useCreateWorktreeMutation,
    useLockWorktreeMutation,
    useRemoveWorktreeMutation,
    useUnlockWorktreeMutation,
} from "@/lib/backend/mutations/repository-mutations";
import { toastError } from "@/lib/toast-error";
import { cn } from "@/lib/utils";

interface Props {
    repoId: number;
}

function shortPath(path: string): string {
    const normalized = path.replaceAll("\\", "/").replace(/\/+$/, "");
    const parts = normalized.split("/");
    return parts.slice(-2).join("/") || normalized;
}

export function WorktreesManager({ repoId }: Props) {
    const worktrees = useWorktrees(repoId);
    const create = useCreateWorktreeMutation(repoId);
    const remove = useRemoveWorktreeMutation(repoId);
    const lock = useLockWorktreeMutation(repoId);
    const unlock = useUnlockWorktreeMutation(repoId);
    const { confirm } = useConfirm();

    const [name, setName] = useState("");
    const [startPoint, setStartPoint] = useState("");
    const [path, setPath] = useState("");

    const trees = worktrees.data ?? [];

    const createTree = async () => {
        const trimmed = name.trim();
        if (!trimmed) return;
        try {
            await create.mutateAsync({
                name: trimmed,
                startPoint: startPoint.trim() || undefined,
                path: path.trim() || undefined,
            });
            setName("");
            setStartPoint("");
            setPath("");
        } catch (error) {
            toastError("Could not create worktree", error);
        }
    };

    const removeTree = async (treeName: string, force: boolean) => {
        const result = await confirm({
            title: `Remove worktree ${treeName}?`,
            description: force
                ? "The checkout directory will be deleted. This cannot be undone."
                : "The tree's admin metadata will be pruned.",
            confirmText: "Remove",
            variant: "destructive",
        });
        if (!result.confirmed) return;
        try {
            await remove.mutateAsync({ name: treeName, force });
        } catch (error) {
            toastError("Could not remove worktree", error);
        }
    };

    return (
        <div className="flex min-h-0 flex-col">
            <div className="flex items-center justify-between px-1 pb-1">
                <span className="text-xs font-medium text-muted-foreground">
                    Worktrees
                </span>
            </div>

            {trees.length === 0 ? (
                <p className="px-1 pb-1 text-xs text-muted-foreground">
                    No linked worktrees.
                </p>
            ) : (
                <div className="flex max-h-56 min-h-0 flex-col overflow-y-auto">
                    {trees.map((tree) => (
                        <WorktreeRow
                            key={tree.name}
                            tree={tree}
                            onLock={() =>
                                tree.lockedBy !== undefined
                                    ? void unlock
                                          .mutateAsync({ name: tree.name })
                                          .catch((error) =>
                                              toastError(
                                                  "Could not unlock worktree",
                                                  error
                                              )
                                          )
                                    : void lock
                                          .mutateAsync({ name: tree.name })
                                          .catch((error) =>
                                              toastError(
                                                  "Could not lock worktree",
                                                  error
                                              )
                                          )
                            }
                            onRemove={() => {
                                const force =
                                    tree.isPrunable ||
                                    tree.lockedBy !== undefined;
                                void removeTree(tree.name, force);
                            }}
                            busy={
                                remove.isPending ||
                                lock.isPending ||
                                unlock.isPending
                            }
                        />
                    ))}
                </div>
            )}

            <Separator className="my-1.5" />
            <div className="flex flex-col gap-1.5">
                <Input
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    placeholder="Worktree name"
                    aria-label="Worktree name"
                />
                <div className="flex items-center gap-1.5">
                    <Input
                        value={startPoint}
                        onChange={(e) => setStartPoint(e.target.value)}
                        placeholder="Branch or commit"
                        aria-label="Start point"
                    />
                    <Input
                        value={path}
                        onChange={(e) => setPath(e.target.value)}
                        placeholder="Path (optional)"
                        aria-label="Worktree path"
                        className="max-w-32"
                    />
                </div>
                <Button
                    variant="secondary"
                    size="sm"
                    className="w-full"
                    disabled={!name.trim() || create.isPending}
                    loading={create.isPending}
                    onClick={() => void createTree()}
                >
                    Create worktree
                </Button>
            </div>
        </div>
    );
}

interface RowProps {
    tree: {
        name: string;
        path: string;
        branch?: string;
        isCurrent: boolean;
        lockedBy?: string;
        isPrunable: boolean;
    };
    onLock: () => void;
    onRemove: () => void;
    busy: boolean;
}

function WorktreeRow({ tree, onLock, onRemove, busy }: RowProps) {
    const locked = tree.lockedBy !== undefined;
    return (
        <div
            className={cn(
                "flex w-full items-center justify-between rounded-sm px-2 py-1 hover:bg-accent",
                tree.isCurrent && "bg-accent"
            )}
        >
            <div className="min-w-0 flex-1">
                <div className="flex items-center gap-1.5">
                    <span className="max-w-44 truncate text-sm font-medium">
                        {tree.name}
                    </span>
                    {tree.isPrunable && (
                        <Badge
                            className="h-5 rounded-md border-destructive/40 px-1.5 font-mono text-sm text-destructive"
                            variant="outline"
                        >
                            prunable
                        </Badge>
                    )}
                </div>
                <div className="flex items-center gap-1.5 truncate text-xs text-muted-foreground">
                    {tree.branch && (
                        <span className="max-w-32 truncate">{tree.branch}</span>
                    )}
                    <span>·</span>
                    <span className="truncate">{shortPath(tree.path)}</span>
                </div>
            </div>
            <div className="flex shrink-0 items-center gap-0.5 pl-2">
                {!tree.isCurrent && (
                    <>
                        <Tooltip>
                            <TooltipTrigger
                                render={
                                    <Button
                                        variant="ghost"
                                        size="icon-sm"
                                        disabled={busy}
                                        aria-label={
                                            locked
                                                ? "Unlock worktree"
                                                : "Lock worktree"
                                        }
                                        onClick={onLock}
                                    >
                                        {locked ? (
                                            <LockOpen className="size-3.5" />
                                        ) : (
                                            <Lock className="size-3.5" />
                                        )}
                                    </Button>
                                }
                            />
                            <TooltipContent>
                                {locked ? "Unlock worktree" : "Lock worktree"}
                            </TooltipContent>
                        </Tooltip>
                        <Tooltip>
                            <TooltipTrigger
                                render={
                                    <Button
                                        variant="ghost"
                                        size="icon-sm"
                                        disabled={busy}
                                        aria-label="Remove worktree"
                                        onClick={onRemove}
                                    >
                                        <Trash2 className="size-3.5" />
                                    </Button>
                                }
                            />
                            <TooltipContent>
                                {tree.isPrunable
                                    ? "Prune worktree"
                                    : "Remove worktree (deletes files)"}
                            </TooltipContent>
                        </Tooltip>
                    </>
                )}
            </div>
        </div>
    );
}
