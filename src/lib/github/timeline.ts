import type {
    GithubIssueComment,
    GithubIssueEvent,
    GithubPullRequestCommit,
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
    | {
          kind: "commit";
          /** The author date, since a commit carries no comment time. */
          createdAt: string;
          commit: GithubPullRequestCommit;
      }
    | {
          kind: "review";
          createdAt: string;
          review: GithubPullRequestReview;
          /** The reviewer's inline comments, nested under their review. */
          comments: GithubPullRequestReviewComment[];
      }
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

/** The opening message leads the thread whatever else says, since it is what
 * the thread is about. Its creation time can sit after a comment that landed
 * on a backdated commit. */
function leadingBody(thread: TimelineThread): {
    kind: "body";
    createdAt: string;
} {
    return { kind: "body", createdAt: thread.createdAt };
}

export function buildIssueTimeline(
    thread: TimelineThread,
    { comments, events }: TimelineParts
): IssueTimelineItem[] {
    const rest = mergeTimeline([
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
    return [leadingBody(thread), ...rest];
}

/** One chronological stream of a pull request: the opening body, then the
 * commits on the head branch, comments, timeline events, and submitted
 * reviews. A review carries its own inline comments.
 *
 * Inline comments are nested under their review rather than merged into the
 * stream: a reviewer creates them before submitting, so their timestamps
 * precede the review and a flat sort puts every suggestion above the summary
 * that introduces it. */ export function buildPullTimeline(
    thread: TimelineThread,
    parts: TimelineParts & {
        commits: GithubPullRequestCommit[];
        reviews: GithubPullRequestReview[];
        reviewComments: GithubPullRequestReviewComment[];
    }
): TimelineItem[] {
    const byCreatedAt = (a: { createdAt: string }, b: { createdAt: string }) =>
        a.createdAt.localeCompare(b.createdAt);

    const orphaned: GithubPullRequestReviewComment[] = [];
    const nested = new Map<number, GithubPullRequestReviewComment[]>();
    for (const comment of [...parts.reviewComments].sort(byCreatedAt)) {
        const reviewId = comment.pullRequestReviewId;
        if (reviewId == null) {
            orphaned.push(comment);
            continue;
        }
        const bucket = nested.get(reviewId);
        if (bucket === undefined) {
            nested.set(reviewId, [comment]);
        } else {
            bucket.push(comment);
        }
    }

    const items: TimelineItem[] = [
        ...parts.comments.map((comment): TimelineItem => ({
            kind: "comment",
            createdAt: comment.createdAt,
            comment,
        })),
        ...parts.commits.map((commit): TimelineItem => ({
            kind: "commit",
            createdAt: commit.authoredAt,
            commit,
        })),
        ...parts.events
            .filter((event) => !HIDDEN_EVENT_KINDS.has(event.kind))
            .map((event): TimelineItem => ({
                kind: "event",
                createdAt: event.createdAt,
                event,
            })),
        ...parts.reviews.map((review): TimelineItem => ({
            kind: "review",
            createdAt: review.submittedAt,
            review,
            comments: nested.get(review.id) ?? [],
        })),
    ];
    for (const comment of orphaned) {
        items.push({
            kind: "reviewComment",
            createdAt: comment.createdAt,
            comment,
        });
    }
    items.sort(byCreatedAt);
    return [leadingBody(thread), ...items];
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
        case "commit":
            return `commit-${item.commit.sha}`;
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
