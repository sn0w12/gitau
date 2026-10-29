import { ChevronDownIcon, GitMerge } from "lucide-react";

import { isOpenThread } from "@/components/forge/status-badge";
import { SidebarBlock } from "@/components/forge/thread-chrome";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Group, GroupSeparator } from "@/components/ui/group";
import { Menu, MenuItem, MenuPopup, MenuTrigger } from "@/components/ui/menu";
import type {
    PullRequestDetail,
    PullRequestMergeMethod,
} from "@/lib/backend/protocol";

const MERGE_METHODS: PullRequestMergeMethod[] = ["merge", "squash", "rebase"];

export const MERGE_METHOD_TEXT: Record<
    PullRequestMergeMethod,
    { label: string; description: string }
> = {
    merge: {
        label: "Create a merge commit",
        description:
            "All commits from this branch will be added to the base branch via a merge commit.",
    },
    squash: {
        label: "Squash and merge",
        description:
            "The 1 commit from this branch will be added to the base branch.",
    },
    rebase: {
        label: "Rebase and merge",
        description:
            "The 1 commit from this branch will be rebased and added to the base branch.",
    },
};

/**
 * Why a merge cannot go ahead, in the provider's own terms. Null when it can.
 */
export function mergeBlocker(pull: PullRequestDetail): string | null {
    if (!isOpenThread(pull)) {
        return "This pull request is closed.";
    }
    if (pull.draft) return "Mark this ready for review to merge it.";
    if (pull.mergeable === null || pull.mergeable === undefined) {
        return "Still working out whether this can merge.";
    }
    if (!pull.mergeable) {
        return pull.mergeableState === "dirty"
            ? "This branch has conflicts with the base branch."
            : "This cannot merge right now.";
    }
    return null;
}

/**
 * The one action a viewer with write access can take on an open pull
 * request, beside the title. A draft offers "ready for review" instead of a
 * merge, because that is the only thing that unblocks it, so the header never
 * shows a dead control next to a live one. Without write access there is
 * nothing to do, and the composer's close/reopen button covers the rest.
 */
export function PullHeaderActions({
    pull,
    canPush,
    onMerge,
    onToggleDraft,
    pending,
}: {
    pull: PullRequestDetail;
    canPush: boolean;
    onMerge: (method: PullRequestMergeMethod) => void;
    onToggleDraft: () => void;
    pending: boolean;
}) {
    const blocker = mergeBlocker(pull);
    if (!canPush || !isOpenThread(pull)) return null;

    if (pull.draft) {
        return (
            <Button loading={pending} onClick={onToggleDraft}>
                Ready for review
            </Button>
        );
    }

    return (
        <div className="flex items-center gap-1.5">
            <Group>
                <Button
                    variant={blocker !== null ? "default" : "info"}
                    disabled={blocker !== null}
                    loading={pending}
                    size="sm"
                    onClick={() => onMerge("merge")}
                >
                    <span className="inline-flex items-center gap-1">
                        <GitMerge />
                        {blocker === null
                            ? "Merge pull request"
                            : "Cannot merge yet"}
                    </span>
                </Button>
                <GroupSeparator />
                <Menu>
                    <MenuTrigger
                        render={
                            <Button
                                aria-label="Copy options"
                                size="icon-sm"
                                variant={blocker !== null ? "default" : "info"}
                                disabled={blocker !== null}
                            />
                        }
                    >
                        <ChevronDownIcon
                            aria-hidden="true"
                            className="size-4"
                        />
                    </MenuTrigger>
                    <MenuPopup align="end" className="max-w-72">
                        {MERGE_METHODS.map((value) => (
                            <MenuItem
                                key={value}
                                onClick={() => onMerge(value)}
                            >
                                <span className="flex flex-col">
                                    {MERGE_METHOD_TEXT[value].label}
                                    <span className="text-xs text-muted-foreground">
                                        {MERGE_METHOD_TEXT[value].description}
                                    </span>
                                </span>
                            </MenuItem>
                        ))}
                    </MenuPopup>
                </Menu>
            </Group>
        </div>
    );
}

/** What the branch changes, as one titled sidebar entry like its
 * neighbours. */
export function PullChangesBlock({ pull }: { pull: PullRequestDetail }) {
    return (
        <SidebarBlock label="Changes">
            <div className="flex flex-wrap items-center gap-1">
                <Badge variant="success">+{pull.additions}</Badge>
                <Badge variant="error">-{pull.deletions}</Badge>
                <span className="text-muted-foreground">
                    {pull.changedFiles}{" "}
                    {pull.changedFiles === 1 ? "file" : "files"}
                </span>
                <span aria-hidden="true" className="text-muted-foreground/50">
                    &middot;
                </span>
                <span className="text-muted-foreground">
                    {pull.commits} {pull.commits === 1 ? "commit" : "commits"}
                </span>
            </div>
        </SidebarBlock>
    );
}
