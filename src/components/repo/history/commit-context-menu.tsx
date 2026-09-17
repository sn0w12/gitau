import {
    Cherry,
    Copy,
    GitBranch,
    GitBranchPlus,
    History,
    PenLine,
    RotateCcw,
    Tag,
    Undo2,
} from "lucide-react";
import type * as React from "react";
import { useState } from "react";

import { AmendCommitDialog } from "@/components/repo/history/amend-commit-dialog";
import { CreateBranchDialog } from "@/components/repo/history/create-branch-dialog";
import { CreateTagDialog } from "@/components/repo/history/create-tag-dialog";
import {
    ContextMenu,
    ContextMenuItem,
    ContextMenuPopup,
    ContextMenuSeparator,
    ContextMenuSub,
    ContextMenuSubPopup,
    ContextMenuSubTrigger,
    ContextMenuTrigger,
} from "@/components/ui/context-menu";
import { useCommitActions } from "@/hooks/repositories/use-commit-actions";
import { useRepositorySnapshot } from "@/hooks/repositories/use-repository-queries";
import type { CommitSummary, ResetKind } from "@/lib/backend/protocol";

type DialogKind = "amend" | "tag" | "branch";

const RESET_KIND_ITEMS: Array<{ kind: ResetKind; label: string }> = [
    { kind: "soft", label: "Soft" },
    { kind: "mixed", label: "Mixed" },
    { kind: "hard", label: "Hard" },
];

/**
 * Right-click menu for a commit row. `render` composes the trigger onto the
 * caller's row element (Base UI render prop) so list semantics stay intact.
 * HEAD-only actions gate on the snapshot's head target.
 */
export function CommitContextMenu({
    repoId,
    commit,
    render,
}: {
    repoId: number;
    commit: CommitSummary;
    render: React.ReactElement;
}): React.ReactElement {
    const [opened, setOpened] = useState(false);

    return (
        <ContextMenu
            onOpenChange={(open) => {
                if (open) setOpened(true);
            }}
        >
            <ContextMenuTrigger render={render} />
            {opened && (
                <CommitContextMenuBody repoId={repoId} commit={commit} />
            )}
        </ContextMenu>
    );
}

/**
 * The menu items and the three dialogs they open. Mounted only after the menu
 * is first opened, because every visible history row otherwise carries a
 * snapshot subscription, seven mutation hooks, and three dialog trees that all
 * re-render on every list render. Once opened it stays mounted so the menu and
 * dialog close transitions can finish.
 */
function CommitContextMenuBody({
    repoId,
    commit,
}: {
    repoId: number;
    commit: CommitSummary;
}): React.ReactElement {
    const snapshot = useRepositorySnapshot(repoId);
    const head = snapshot.data?.head;
    const headTarget =
        head?.state === "attached" || head?.state === "detached"
            ? head.target
            : null;
    const isHead = headTarget === commit.id;

    const actions = useCommitActions(repoId);
    const busy = actions.busy;

    const [dialog, setDialog] = useState<DialogKind | null>(null);

    return (
        <>
            <ContextMenuPopup>
                <ContextMenuItem
                    disabled={busy || !isHead}
                    onClick={() => setDialog("amend")}
                    data-testid="commit-menu-amend"
                >
                    <PenLine />
                    Amend…
                </ContextMenuItem>
                <ContextMenuItem
                    disabled={busy || !isHead || commit.parentIds.length === 0}
                    onClick={() => void actions.undoCommit(commit)}
                    data-testid="commit-menu-undo"
                >
                    <Undo2 />
                    Undo commit
                </ContextMenuItem>
                <ContextMenuSeparator />
                <ContextMenuItem
                    disabled={busy || isHead}
                    onClick={() => void actions.checkoutCommit(commit)}
                    data-testid="commit-menu-checkout"
                >
                    <GitBranch />
                    Checkout
                </ContextMenuItem>
                <ContextMenuSeparator />
                <ContextMenuSub>
                    <ContextMenuSubTrigger
                        disabled={busy}
                        data-testid="commit-menu-reset"
                    >
                        <History />
                        Reset to…
                    </ContextMenuSubTrigger>
                    <ContextMenuSubPopup>
                        {RESET_KIND_ITEMS.map(({ kind, label }) => (
                            <ContextMenuItem
                                key={kind}
                                variant={
                                    kind === "hard" ? "destructive" : "default"
                                }
                                onClick={() =>
                                    void actions.resetTo(commit, kind)
                                }
                                data-testid={`commit-menu-reset-${kind}`}
                            >
                                {label}
                            </ContextMenuItem>
                        ))}
                    </ContextMenuSubPopup>
                </ContextMenuSub>
                <ContextMenuItem
                    disabled={busy}
                    onClick={() => void actions.revertCommit(commit)}
                    data-testid="commit-menu-revert"
                >
                    <RotateCcw />
                    Revert
                </ContextMenuItem>
                <ContextMenuItem
                    disabled={busy}
                    onClick={() => void actions.cherryPickCommit(commit)}
                    data-testid="commit-menu-cherry-pick"
                >
                    <Cherry />
                    Cherry-pick
                </ContextMenuItem>
                <ContextMenuSeparator />
                <ContextMenuItem
                    disabled={busy}
                    onClick={() => setDialog("branch")}
                    data-testid="commit-menu-create-branch"
                >
                    <GitBranchPlus />
                    Create branch here…
                </ContextMenuItem>
                <ContextMenuItem
                    disabled={busy}
                    onClick={() => setDialog("tag")}
                    data-testid="commit-menu-create-tag"
                >
                    <Tag />
                    Create tag…
                </ContextMenuItem>
                <ContextMenuSeparator />
                <ContextMenuItem onClick={() => actions.copyMessage(commit)}>
                    <Copy />
                    Copy message
                </ContextMenuItem>
                <ContextMenuItem
                    onClick={() => actions.copySha(commit)}
                    data-testid="commit-menu-copy-sha"
                >
                    <Copy />
                    Copy SHA
                </ContextMenuItem>
            </ContextMenuPopup>
            <AmendCommitDialog
                commit={commit}
                open={dialog === "amend"}
                onSubmit={(message) => actions.amendCommit(message)}
                onClose={() => setDialog(null)}
            />
            <CreateTagDialog
                commit={commit}
                open={dialog === "tag"}
                onSubmit={(input) => actions.createTagHere(commit, input)}
                onClose={() => setDialog(null)}
            />
            <CreateBranchDialog
                commit={commit}
                open={dialog === "branch"}
                onSubmit={(name) => actions.createBranchHere(commit, name)}
                onClose={() => setDialog(null)}
            />
        </>
    );
}
