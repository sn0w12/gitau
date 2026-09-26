import { Badge } from "@/components/ui/badge";

export type GithubIssueStatus = "closed" | "open";
const statusLabel: Record<GithubIssueStatus, string> = {
    closed: "Closed",
    open: "Open",
};

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
        <Badge
            variant={status === "closed" ? "info" : "success"}
            className={className}
        >
            {statusLabel[status]}
        </Badge>
    );
}
