import { GitMerge } from "lucide-react";

import { Badge } from "@/components/ui/badge";

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

const statusVariant: Record<ThreadStatus, "info" | "secondary" | "success"> = {
    closed: "secondary",
    draft: "secondary",
    merged: "info",
    open: "success",
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
            {status === "merged" ? <GitMerge /> : null}
            {statusLabel[status]}
        </Badge>
    );
}
