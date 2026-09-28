import { CheckCheck, MessageSquareWarning, X } from "lucide-react";

import { CodeBlock } from "@/components/github/code-block";
import { TimelineMessage } from "@/components/github/message";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Frame, FramePanel } from "@/components/ui/frame";
import type {
    GithubPullRequestReview,
    GithubPullRequestReviewComment,
} from "@/lib/backend/protocol";
import { formatRelativeDate } from "@/lib/utils";

function initials(login: string): string {
    return login.slice(0, 1).toUpperCase() || "?";
}

/** GitHub's review verdicts. `PENDING` never reaches the submitted-reviews
 * list, so it is not handled. */
function reviewAppearance(state: string): {
    icon: React.ReactNode;
    label: string;
    className: string;
} {
    switch (state.toUpperCase()) {
        case "APPROVED":
            return {
                icon: <CheckCheck className="size-4" />,
                label: "approved these changes",
                className: "text-success",
            };
        case "CHANGES_REQUESTED":
            return {
                icon: <MessageSquareWarning className="size-4" />,
                label: "requested changes",
                className: "text-destructive",
            };
        case "DISMISSED":
            return {
                icon: <X className="size-4" />,
                label: "dismissed their review",
                className: "text-muted-foreground",
            };
        default:
            return {
                icon: <MessageSquareWarning className="size-4" />,
                label: "left a review in progress",
                className: "text-muted-foreground",
            };
    }
}

export function TimelineReview({
    review,
    owner,
    repo,
    canQuote,
    onQuote,
}: {
    review: GithubPullRequestReview;
    owner: string;
    repo: string;
    canQuote: boolean;
    onQuote: (text: string) => void;
}) {
    const { icon, label, className } = reviewAppearance(review.state);

    return (
        <div className="w-full">
            <div className="ui-selectable flex items-center gap-1 px-1.5 py-1 text-sm">
                <div
                    className={`flex size-6 items-center justify-center rounded-full bg-muted ${className}`}
                >
                    {icon}
                </div>
                <Avatar className="size-6">
                    <AvatarImage src={review.author.avatarUrl || undefined} />
                    <AvatarFallback>
                        {initials(review.author.login)}
                    </AvatarFallback>
                </Avatar>
                <span>
                    {review.author.login}{" "}
                    <span className="text-muted-foreground">
                        {label} {formatRelativeDate(review.submittedAt)}
                    </span>
                </span>
            </div>
            {review.body ? (
                <TimelineMessage
                    text={review.body}
                    owner={owner}
                    repo={repo}
                    canQuote={canQuote}
                    onQuote={onQuote}
                    link={review.htmlUrl || undefined}
                />
            ) : null}
        </div>
    );
}

/** An inline comment, with the diff hunk it was written against so the
 * comment makes sense without leaving the page. */
export function TimelineReviewComment({
    comment,
    owner,
    repo,
    canQuote,
    onQuote,
}: {
    comment: GithubPullRequestReviewComment;
    owner: string;
    repo: string;
    canQuote: boolean;
    onQuote: (text: string) => void;
}) {
    return (
        <div className="w-full">
            <div className="ui-selectable flex items-center gap-1.5 px-1.5 py-1 text-sm">
                <Avatar className="size-5">
                    <AvatarImage src={comment.author.avatarUrl || undefined} />
                    <AvatarFallback>
                        {initials(comment.author.login)}
                    </AvatarFallback>
                </Avatar>
                <span className="font-mono text-xs text-muted-foreground">
                    {comment.path}
                    {comment.line != null ? `:${comment.line}` : ""}
                </span>
                <span className="truncate text-xs text-muted-foreground">
                    {comment.author.login}{" "}
                    {formatRelativeDate(comment.createdAt)}
                </span>
            </div>
            {comment.diffHunk ? (
                <Frame className="mx-4 w-auto">
                    <FramePanel className="px-0 py-0">
                        <CodeBlock language="diff" text={comment.diffHunk} />
                    </FramePanel>
                </Frame>
            ) : null}
            <TimelineMessage
                text={comment.body}
                owner={owner}
                repo={repo}
                canQuote={canQuote}
                onQuote={onQuote}
                link={comment.htmlUrl || undefined}
                actionLabel="commented on"
            />
        </div>
    );
}

/** Compact list of who reviewed and with what verdict, for the sidebar. A
 * reviewer can submit more than once, so their latest verdict wins. */
export function ReviewSummary({
    reviews,
}: {
    reviews: GithubPullRequestReview[];
}) {
    if (reviews.length === 0) {
        return <span className="text-sm text-muted-foreground">None</span>;
    }
    const latest = new Map<string, GithubPullRequestReview>();
    for (const review of reviews) {
        latest.set(review.author.login, review);
    }
    return (
        <ul className="flex flex-col gap-0.5">
            {[...latest.values()].map((review) => {
                const { icon, label, className } = reviewAppearance(
                    review.state
                );
                return (
                    <li
                        key={review.author.login}
                        className="flex items-center gap-1.5"
                    >
                        <span className={className}>{icon}</span>
                        <span className="truncate text-sm">
                            {review.author.login}
                        </span>
                        <span className="truncate text-xs text-muted-foreground">
                            {label}
                        </span>
                    </li>
                );
            })}
        </ul>
    );
}
