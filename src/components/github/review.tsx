import {
    CheckCheck,
    ChevronRight,
    MessageSquareWarning,
    X,
} from "lucide-react";
import { Fragment } from "react";

import { CodeBlock } from "@/components/github/code-block";
import { MessageSpacer, TimelineMessage } from "@/components/github/message";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import {
    Collapsible,
    CollapsiblePanel,
    CollapsibleTrigger,
} from "@/components/ui/collapsible";
import type {
    GithubPullRequestReview,
    GithubPullRequestReviewComment,
} from "@/lib/backend/protocol";
import { formatRelativeDate } from "@/lib/utils";

function initials(login: string): string {
    return login.slice(0, 1).toUpperCase() || "?";
}

/** GitHub's review verdicts. `PENDING` never reaches the submitted-reviews
 * list, so the default branch only covers states GitHub adds later. */
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
        case "COMMENTED":
            return {
                icon: <MessageSquareWarning className="size-4" />,
                label: "reviewed",
                className: "text-muted-foreground",
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
    comments,
    owner,
    repo,
    canQuote,
    onQuote,
}: {
    review: GithubPullRequestReview;
    /** The reviewer's inline comments, nested under the review that
     * introduced them. */
    comments: GithubPullRequestReviewComment[];
    owner: string;
    repo: string;
    canQuote: boolean;
    onQuote: (text: string) => void;
}) {
    const { icon, label, className } = reviewAppearance(review.state);

    return (
        <>
            <div className="w-full">
                <MessageSpacer />
                <div className="ui-selectable flex items-center gap-1 px-1.5 py-1 text-sm">
                    <div
                        className={`flex size-6.5 items-center justify-center rounded-full bg-muted ${className}`}
                    >
                        {icon}
                    </div>
                    <Avatar className="size-6.5">
                        <AvatarImage
                            src={review.author.avatarUrl || undefined}
                        />
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
                    <>
                        <MessageSpacer />
                        <TimelineMessage
                            text={review.body}
                            owner={owner}
                            repo={repo}
                            canQuote={canQuote}
                            onQuote={onQuote}
                            link={review.htmlUrl || undefined}
                        />
                    </>
                ) : null}
                {comments.map((comment) => (
                    <Fragment key={comment.id}>
                        <MessageSpacer />
                        <TimelineReviewComment
                            comment={comment}
                            owner={owner}
                            repo={repo}
                            canQuote={canQuote}
                            onQuote={onQuote}
                        />
                    </Fragment>
                ))}
            </div>
        </>
    );
}

/** The fence language for a file path, so a review comment's hunk is
 * highlighted as the language it is written in rather than as a diff. */
function languageFromPath(path: string): string {
    const name = path.split("/").pop() ?? "";
    const dot = name.lastIndexOf(".");
    return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
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
        <div className="flex flex-col gap-2">
            <div className="w-full">
                <div className="ui-selectable flex items-center gap-1.5 px-2 pt-2 pb-1.5 text-sm">
                    <Avatar className="size-5.5">
                        <AvatarImage
                            src={comment.author.avatarUrl || undefined}
                        />
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
                <MessageSpacer />
                <TimelineMessage
                    text={comment.body}
                    owner={owner}
                    repo={repo}
                    canQuote={canQuote}
                    onQuote={onQuote}
                    link={comment.htmlUrl || undefined}
                    showAuthor={false}
                />
                {comment.diffHunk ? (
                    <div className="grid grid-cols-50">
                        <MessageSpacer className="h-stretch" />
                        <div className="col-span-49 mt-2 ml-4 w-auto">
                            <Collapsible>
                                <CollapsibleTrigger className="flex items-center gap-1.5 rounded-sm px-0.5 py-0.5 text-xs text-muted-foreground hover:text-foreground [&[data-panel-open]>svg]:rotate-90">
                                    <ChevronRight className="size-3.5 shrink-0 transition-transform" />
                                    Show the diff for this comment
                                </CollapsibleTrigger>
                                <CollapsiblePanel>
                                    <CodeBlock
                                        language={languageFromPath(
                                            comment.path
                                        )}
                                        text={comment.diffHunk}
                                        diff
                                    />
                                </CollapsiblePanel>
                            </Collapsible>
                        </div>
                    </div>
                ) : null}
            </div>
        </div>
    );
}

/** Whether a review carries a verdict, as opposed to being a comment-only
 * review. A later `COMMENTED` never displaces a verdict the reviewer already
 * gave. */
function isVerdict(state: string): boolean {
    const verdict = state.toUpperCase();
    return (
        verdict === "APPROVED" ||
        verdict === "CHANGES_REQUESTED" ||
        verdict === "DISMISSED"
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
        const seen = latest.get(review.author.login);
        if (seen && isVerdict(seen.state) && !isVerdict(review.state)) {
            continue;
        }
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
