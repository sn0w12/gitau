import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

export type GithubIssueStatus = "closed" | "open";
const statusColor: Record<GithubIssueStatus, string> = {
    closed: "bg-info",
    open: "bg-success",
};
const statusLabel: Record<GithubIssueStatus, string> = {
    closed: "Closed",
    open: "Open",
};

/** Accepts raw GitHub state strings; anything not "open" reads as closed. */
export function issueStatusOf(state: string): GithubIssueStatus {
    return state.toLowerCase() === "open" ? "open" : "closed";
}

export function StatusBadge({
    status,
    className,
}: {
    status: GithubIssueStatus;
    className?: string;
}) {
    return (
        <Badge variant="outline" className={className}>
            <span
                aria-hidden="true"
                className={cn("size-1.5 rounded-full", statusColor[status])}
            />
            {statusLabel[status]}
        </Badge>
    );
}
