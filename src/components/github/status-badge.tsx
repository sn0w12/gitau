import { CircleDashed, CircleDot, CircleSlash, GitMerge } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

/**
 * Conversation state of a GitHub thread. Issues only ever report `open` or
 * `closed`; pull requests add `merged` and `draft`.
 */
export type ThreadStatus = "closed" | "draft" | "merged" | "open";

const statusLabel: Record<ThreadStatus, string> = {
    closed: "Closed",
    draft: "Draft",
    merged: "Merged",
    open: "Open",
};

/** One glyph per state, so a status reads without its label. The open dot
 * matches the issue rows, and draft reads as unfinished rather than
 * rejected. */
const statusIcon: Record<ThreadStatus, React.ReactNode> = {
    closed: <CircleSlash />,
    draft: <CircleDashed />,
    merged: <GitMerge />,
    open: <CircleDot />,
};

const statusVariant: Record<ThreadStatus, "info" | "secondary" | "success"> = {
    closed: "secondary",
    draft: "secondary",
    merged: "info",
    open: "success",
};

/** Color marks how a thread ended, not just whether it is live: a merge
 * landed and stays blue, while closed and draft recede to gray. */
const statusColor: Record<ThreadStatus, string> = {
    closed: "text-muted-foreground",
    draft: "text-muted-foreground",
    merged: "text-info",
    open: "text-success",
};

export function issueStatusOf(state: string): ThreadStatus {
    return state.toLowerCase() === "open" ? "open" : "closed";
}

/** A merged pull request is reported as `state: "closed"`, so the merge
 * timestamp is the only signal that separates the two, and the draft flag
 * outranks the open state. */
export function pullRequestStatusOf(pull: {
    draft?: boolean;
    mergedAt?: string | null;
    state: string;
}): ThreadStatus {
    if (pull.mergedAt) return "merged";
    if (pull.draft) return "draft";
    return issueStatusOf(pull.state);
}

export function StatusBadge({
    status,
    className,
}: {
    status: ThreadStatus;
    className?: string;
}) {
    return (
        <Badge variant={statusVariant[status]} className={className}>
            {statusIcon[status]}
            {statusLabel[status]}
        </Badge>
    );
}

/**
 * Status as a bare glyph, for thread rows that lead with an icon instead of
 * a badge. The size rule matches the issue rows so every list aligns.
 */
export function ThreadStatusIcon({
    status,
    className,
}: {
    status: ThreadStatus;
    className?: string;
}) {
    return (
        <span
            className={cn(
                "shrink-0",
                statusColor[status],
                "[&_svg:not([class*='size-'])]:size-4",
                className
            )}
        >
            {statusIcon[status]}
        </span>
    );
}
