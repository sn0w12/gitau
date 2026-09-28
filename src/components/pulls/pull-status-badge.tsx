import { GitMerge } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import type { GithubPullRequestListItem } from "@/lib/backend/protocol";

export type PullRequestStatus = "closed" | "merged" | "open";

const statusLabel: Record<PullRequestStatus, string> = {
    closed: "Closed",
    merged: "Merged",
    open: "Open",
};

const statusVariant: Record<
    PullRequestStatus,
    "info" | "secondary" | "success"
> = {
    closed: "secondary",
    merged: "info",
    open: "success",
};

/** GitHub reports a merged pull request as `state: "closed"`, so
 * `merged_at` is the only way to tell the two apart. */
export function pullRequestStatusOf(
    pull: Pick<GithubPullRequestListItem, "mergedAt" | "state">
): PullRequestStatus {
    if (pull.mergedAt) return "merged";
    return pull.state.toLowerCase() === "open" ? "open" : "closed";
}

export function PullStatusBadge({
    status,
    className,
}: {
    status: PullRequestStatus;
    className?: string;
}) {
    return (
        <Badge variant={statusVariant[status]} className={className}>
            {status === "merged" ? <GitMerge /> : null}
            {statusLabel[status]}
        </Badge>
    );
}
