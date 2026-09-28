import type {
    GithubIssueComment,
    GithubIssueEvent,
    GithubPullRequestReview,
    GithubPullRequestReviewComment,
    GithubUser,
} from "@/lib/backend/protocol";

export interface TimelineThread {
    body: string;
    createdAt: string;
}

export type TimelineItem =
    | { kind: "body"; createdAt: string }
    | { kind: "comment"; createdAt: string; comment: GithubIssueComment }
    | { kind: "event"; createdAt: string; event: GithubIssueEvent }
    | { kind: "review"; createdAt: string; review: GithubPullRequestReview }
    | {
          kind: "reviewComment";
          createdAt: string;
          comment: GithubPullRequestReviewComment;
      };

export interface TimelineParts {
    comments: GithubIssueComment[];
    events: GithubIssueEvent[];
}

/** The kinds an issue thread can produce. Pull requests add reviews, so they
 * get the wider [`TimelineItem`]. */
export type IssueTimelineItem = Extract<
    TimelineItem,
    { kind: "body" | "comment" | "event" }
>;

/**
 * Events GitHub records that add nothing to a rendered thread. The review
 * kinds are excluded because reviews and their inline comments arrive as
 * their own rows, so keeping the raw event would show the same thing twice.
 */
const HIDDEN_EVENT_KINDS = new Set([
    "subscribed",
    "unsubscribed",
    "mentioned",
    "referenced",
    "cross-referenced",
    "reviewed",
    "commented",
    "line-commented",
]);

/** Merges the per-kind groups into one stream ordered by timestamp. */
function mergeTimeline(groups: TimelineItem[][]): TimelineItem[] {
    return groups.flat().sort((a, b) => a.createdAt.localeCompare(b.createdAt));
}

export function buildIssueTimeline(
    thread: TimelineThread,
    { comments, events }: TimelineParts
): IssueTimelineItem[] {
    return mergeTimeline([
        [{ kind: "body", createdAt: thread.createdAt }],
        comments.map((comment): TimelineItem => ({
            kind: "comment",
            createdAt: comment.createdAt,
            comment,
        })),
        events
            .filter((event) => !HIDDEN_EVENT_KINDS.has(event.kind))
            .map((event): TimelineItem => ({
                kind: "event",
                createdAt: event.createdAt,
                event,
            })),
    ]) as IssueTimelineItem[];
}

/** One chronological stream of a pull request: the opening body, then
 * comments, timeline events, submitted reviews, and the inline diff
 * comments, all ordered by timestamp. */
export function buildPullTimeline(
    thread: TimelineThread,
    parts: TimelineParts & {
        reviews: GithubPullRequestReview[];
        reviewComments: GithubPullRequestReviewComment[];
    }
): TimelineItem[] {
    return mergeTimeline([
        [{ kind: "body", createdAt: thread.createdAt }],
        parts.comments.map((comment): TimelineItem => ({
            kind: "comment",
            createdAt: comment.createdAt,
            comment,
        })),
        parts.events
            .filter((event) => !HIDDEN_EVENT_KINDS.has(event.kind))
            .map((event): TimelineItem => ({
                kind: "event",
                createdAt: event.createdAt,
                event,
            })),
        parts.reviews.map((review): TimelineItem => ({
            kind: "review",
            createdAt: review.submittedAt,
            review,
        })),
        parts.reviewComments.map((comment): TimelineItem => ({
            kind: "reviewComment",
            createdAt: comment.createdAt,
            comment,
        })),
    ]);
}

export function isTimelineMessage(item: TimelineItem): boolean {
    return item.kind === "body" || item.kind === "comment";
}

/** Stable React key per timeline row, so a re-render keeps the same DOM node
 * for a given comment, event, or review. */
export function timelineKey(item: TimelineItem): string {
    switch (item.kind) {
        case "body":
            return "body";
        case "comment":
            return `comment-${item.comment.id}`;
        case "event":
            return `event-${item.event.id}`;
        case "review":
            return `review-${item.review.id}`;
        case "reviewComment":
            return `review-comment-${item.comment.id}`;
    }
}

/** Merges a thread's participants with everyone who commented, order
 * preserved and deduped by login, because the backend list and the comment
 * list overlap. */
export function timelineParticipants(
    participants: GithubUser[],
    comments: { author: GithubUser }[],
    extra: GithubUser[] = []
): GithubUser[] {
    const seen = new Set<string>();
    const out: GithubUser[] = [];
    for (const user of [
        ...participants,
        ...comments.map((comment) => comment.author),
        ...extra,
    ]) {
        const key = user.login.toLowerCase();
        if (key === "" || seen.has(key)) continue;
        seen.add(key);
        out.push(user);
    }
    return out;
}
