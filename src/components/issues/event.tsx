import { CircleCheck, CircleDot, Tag, UserRound } from "lucide-react";

import { LabelBadge } from "@/components/repo/issues/issues-view";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import type { GithubLabel } from "@/lib/backend/protocol";
import { formatRelativeDate } from "@/lib/utils";

function EventIcon({ kind }: { kind: string }) {
    switch (kind) {
        case "closed":
            return <CircleCheck className="size-4" />;
        case "reopened":
            return <CircleDot className="size-4" />;
        case "assigned":
        case "unassigned":
            return <UserRound className="size-4" />;
        default:
            return <Tag className="size-4" />;
    }
}

function eventText(kind: string): string {
    switch (kind) {
        case "labeled":
            return "added";
        case "unlabeled":
            return "removed";
        case "assigned":
            return "assigned";
        case "unassigned":
            return "unassigned";
        case "closed":
            return "closed this";
        case "reopened":
            return "reopened this";
        default:
            return kind.replaceAll("_", " ");
    }
}

export function IssueEvent({
    last,
    kind = "labeled",
    actor,
    avatarUrl,
    createdAt,
    label,
    labelColor,
    assignee,
}: {
    last?: boolean;
    kind?: string;
    actor?: string;
    avatarUrl?: string;
    createdAt?: string;
    label?: string;
    labelColor?: string;
    assignee?: string;
}) {
    const initial = (actor ?? "?").slice(0, 1).toUpperCase();
    return (
        <>
            <div className="ml-4 h-1.5 w-0.5 bg-muted" />
            <div className="ui-selectable flex items-center gap-1 px-1.5 py-1 text-sm">
                <div className="flex size-6 items-center justify-center rounded-full bg-primary text-background">
                    <EventIcon kind={kind} />
                </div>
                <Avatar className="size-6">
                    <AvatarImage src={avatarUrl} />
                    <AvatarFallback>{initial}</AvatarFallback>
                </Avatar>
                <span>
                    {actor ?? "Someone"}{" "}
                    <span className="text-muted-foreground">
                        {eventText(kind)}
                    </span>
                </span>
                {label ? (
                    <LabelBadge
                        label={
                            {
                                name: label,
                                color: labelColor ?? "",
                            } satisfies GithubLabel
                        }
                    />
                ) : null}
                {assignee && !label ? <span>{assignee}</span> : null}
                {createdAt ? (
                    <span className="text-muted-foreground">
                        on {formatRelativeDate(createdAt)}
                    </span>
                ) : null}
            </div>
            {!last && <div className="ml-4 h-1.5 w-0.5 bg-muted" />}
        </>
    );
}
