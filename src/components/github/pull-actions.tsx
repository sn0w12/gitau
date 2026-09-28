import { GitMerge } from "lucide-react";
import { useState } from "react";

import { issueStatusOf } from "@/components/github/status-badge";
import { SidebarBlock } from "@/components/github/thread-chrome";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
    Select,
    SelectItem,
    SelectPopup,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import type {
    GithubPullRequestDetail,
    PullRequestMergeMethod,
} from "@/lib/backend/protocol";

const MERGE_METHODS: PullRequestMergeMethod[] = ["merge", "squash", "rebase"];

export const MERGE_METHOD_LABEL: Record<PullRequestMergeMethod, string> = {
    merge: "Create a merge commit",
    squash: "Squash and merge",
    rebase: "Rebase and merge",
};

/**
 * Why a merge cannot go ahead, in GitHub's own terms. Null when it can.
 */
export function mergeBlocker(pull: GithubPullRequestDetail): string | null {
    if (issueStatusOf(pull.state) !== "open") {
        return "This pull request is closed.";
    }
    if (pull.draft) return "Mark this ready for review to merge it.";
    if (pull.mergeable === null || pull.mergeable === undefined) {
        return "GitHub is still working out whether this can merge.";
    }
    if (!pull.mergeable) {
        return pull.mergeableState === "dirty"
            ? "This branch has conflicts with the base branch."
            : "GitHub reports this cannot merge right now.";
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
    pull: GithubPullRequestDetail;
    canPush: boolean;
    onMerge: (method: PullRequestMergeMethod) => void;
    onToggleDraft: () => void;
    pending: boolean;
}) {
    const [method, setMethod] = useState<PullRequestMergeMethod>("merge");
    const blocker = mergeBlocker(pull);

    if (!canPush || issueStatusOf(pull.state) !== "open") return null;

    if (pull.draft) {
        return (
            <Button loading={pending} onClick={onToggleDraft}>
                Ready for review
            </Button>
        );
    }

    return (
        <div className="flex items-center gap-1.5">
            {blocker === null ? (
                <Select
                    aria-label="Merge method"
                    value={method}
                    onValueChange={(next) => {
                        if (typeof next === "string") {
                            setMethod(next as PullRequestMergeMethod);
                        }
                    }}
                    items={MERGE_METHODS.map((value) => ({
                        value,
                        label: MERGE_METHOD_LABEL[value],
                    }))}
                >
                    <SelectTrigger className="w-auto">
                        <SelectValue />
                    </SelectTrigger>
                    <SelectPopup>
                        {MERGE_METHODS.map((value) => (
                            <SelectItem key={value} value={value}>
                                <span className="truncate">
                                    {MERGE_METHOD_LABEL[value]}
                                </span>
                            </SelectItem>
                        ))}
                    </SelectPopup>
                </Select>
            ) : null}
            <Button
                variant={blocker !== null ? "default" : "info"}
                disabled={blocker !== null}
                loading={pending}
                onClick={() => onMerge(method)}
            >
                <span className="inline-flex items-center gap-1">
                    <GitMerge />
                    {blocker === null
                        ? "Merge pull request"
                        : "Cannot merge yet"}
                </span>
            </Button>
        </div>
    );
}

/** What the branch changes, as one titled sidebar entry like its
 * neighbours. */
export function PullChangesBlock({ pull }: { pull: GithubPullRequestDetail }) {
    return (
        <SidebarBlock label="Changes">
            <div className="flex flex-wrap items-center gap-1">
                <Badge size="sm" variant="success">
                    +{pull.additions}
                </Badge>
                <Badge size="sm" variant="error">
                    -{pull.deletions}
                </Badge>
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
