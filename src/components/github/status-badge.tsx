import {
    CircleCheck,
    CircleDashed,
    CircleDot,
    CircleSlash,
    CopyMinus,
    GitMerge,
} from "lucide-react";

import { Badge } from "@/components/ui/badge";
import type { ThreadStateReason } from "@/lib/backend/protocol";
import { cn } from "@/lib/utils";

/**
 * Conversation state of a GitHub thread. `state` alone is only two values, so
 * the close reason is folded in: a thread closed as fixed does not read the
 * same as one closed as stale, and pull requests add `merged` and `draft`.
 */
export type ThreadStatus =
    | "closed"
    | "completed"
    | "duplicate"
    | "draft"
    | "merged"
    | "notPlanned"
    | "open";

const statusLabel: Record<ThreadStatus, string> = {
    closed: "Closed",
    completed: "Completed",
    duplicate: "Duplicate",
    draft: "Draft",
    merged: "Merged",
    notPlanned: "Not planned",
    open: "Open",
};

/** One glyph per state, so a status reads without its label. Completed and
 * merged both say the work landed, so they are colored alike and told apart
 * by glyph. */
const statusIcon: Record<ThreadStatus, React.ReactNode> = {
    closed: <CircleSlash />,
    completed: <CircleCheck />,
    duplicate: <CopyMinus />,
    draft: <CircleDashed />,
    merged: <GitMerge />,
    notPlanned: <CircleSlash />,
    open: <CircleDot />,
};

const statusVariant: Record<ThreadStatus, "info" | "secondary" | "success"> = {
    closed: "secondary",
    completed: "info",
    duplicate: "secondary",
    draft: "secondary",
    merged: "info",
    notPlanned: "secondary",
    open: "success",
};

/** Color marks how a thread ended, not just whether it is live: work that
 * landed, a merge or a completed issue, stays blue, everything else
 * recedes to gray. */
const statusColor: Record<ThreadStatus, string> = {
    closed: "text-muted-foreground",
    completed: "text-info",
    duplicate: "text-muted-foreground",
    draft: "text-muted-foreground",
    merged: "text-info",
    notPlanned: "text-muted-foreground",
    open: "text-success",
};

/** Only ever two states, whatever the reason says. */
export function isOpenThread(thread: { state: string }): boolean {
    return thread.state.toLowerCase() === "open";
}

/** A reason only means something once the thread is closed; an open issue
 * still carries the reason it was reopened with. */
function closedStatusOf(thread: {
    state: string;
    stateReason?: ThreadStateReason;
}): ThreadStatus {
    if (isOpenThread(thread)) return "open";
    switch (thread.stateReason) {
        case "completed":
            return "completed";
        case "notPlanned":
            return "notPlanned";
        case "duplicate":
            return "duplicate";
        default:
            return "closed";
    }
}

export function issueStatusOf(thread: {
    state: string;
    stateReason?: ThreadStateReason;
}): ThreadStatus {
    return closedStatusOf(thread);
}

/** A merged pull request is reported as `state: "closed"`, so the merge
 * timestamp is the only signal that separates the two, and the draft flag
 * outranks the open state. */
export function pullRequestStatusOf(pull: {
    draft?: boolean;
    mergedAt?: string | null;
    state: string;
    stateReason?: ThreadStateReason;
}): ThreadStatus {
    if (pull.mergedAt) return "merged";
    if (pull.draft) return "draft";
    return closedStatusOf(pull);
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
