import { describe, expect, it } from "vitest";

import type {
    GithubIssueComment,
    GithubIssueEvent,
    GithubPullRequestCommit,
    GithubPullRequestReview,
    GithubPullRequestReviewComment,
    GithubUser,
} from "@/lib/backend/protocol";
import {
    buildIssueTimeline,
    buildPullTimeline,
    isTimelineMessage,
    timelineKey,
    timelineParticipants,
} from "@/lib/github/timeline";

const THREAD = { body: "why", createdAt: "2026-09-01T00:00:00Z" };

function user(login: string): GithubUser {
    return { login, avatarUrl: "" };
}

function comment(
    id: number,
    createdAt: string,
    login = "hubot"
): GithubIssueComment {
    return {
        id,
        author: user(login),
        body: "c",
        createdAt,
        htmlUrl: "",
    };
}

function event(kind: string, createdAt: string): GithubIssueEvent {
    return {
        id: createdAt.length,
        kind,
        actor: "octocat",
        actorAvatarUrl: "",
        createdAt,
    };
}

function review(
    id: number,
    submittedAt: string,
    login = "hubot"
): GithubPullRequestReview {
    return {
        id,
        author: user(login),
        state: "APPROVED",
        body: "",
        submittedAt,
        htmlUrl: "",
    };
}

function reviewComment(
    id: number,
    createdAt: string
): GithubPullRequestReviewComment {
    return {
        id,
        author: user("hubot"),
        body: "nit",
        path: "src/lib.rs",
        line: 4,
        diffHunk: "@@ -1 +1 @@",
        createdAt,
        htmlUrl: "",
    };
}

function commit(sha: string, authoredAt: string): GithubPullRequestCommit {
    return {
        sha,
        message: `work on ${sha}`,
        author: user("hubot"),
        authoredAt,
        htmlUrl: "",
    };
}

describe("buildIssueTimeline", () => {
    it("orders the body, comments, and events by timestamp", () => {
        const timeline = buildIssueTimeline(THREAD, {
            comments: [
                comment(2, "2026-09-03T00:00:00Z"),
                comment(1, "2026-09-02T00:00:00Z"),
            ],
            events: [event("closed", "2026-09-04T00:00:00Z")],
        });
        expect(timeline.map((item) => item.kind)).toEqual([
            "body",
            "comment",
            "comment",
            "event",
        ]);
    });

    it("drops noise events GitHub records but the UI never shows", () => {
        const timeline = buildIssueTimeline(THREAD, {
            comments: [],
            events: [
                event("subscribed", "2026-09-02T00:00:00Z"),
                event("mentioned", "2026-09-02T00:00:00Z"),
                event("labeled", "2026-09-02T00:00:00Z"),
            ],
        });
        expect(timeline.map((item) => item.kind)).toEqual(["body", "event"]);
    });

    /** Reviews arrive from their own endpoint, so the raw `reviewed` event
     * would render the same thing a second time. */
    it("drops the reviewed event that the review rows already cover", () => {
        const timeline = buildIssueTimeline(THREAD, {
            comments: [],
            events: [event("reviewed", "2026-09-02T00:00:00Z")],
        });
        expect(timeline).toHaveLength(1);
    });
});

describe("buildPullTimeline", () => {
    it("interleaves reviews and inline comments with the conversation", () => {
        const timeline = buildPullTimeline(THREAD, {
            commits: [],
            comments: [comment(1, "2026-09-02T00:00:00Z")],
            events: [event("merged", "2026-09-05T00:00:00Z")],
            reviews: [review(7, "2026-09-03T00:00:00Z")],
            reviewComments: [reviewComment(8, "2026-09-04T00:00:00Z")],
        });
        expect(timeline.map((item) => item.kind)).toEqual([
            "body",
            "comment",
            "review",
            "reviewComment",
            "event",
        ]);
    });

    it("interleaves the head branch's commits into the conversation", () => {
        const timeline = buildPullTimeline(THREAD, {
            commits: [
                commit("aaa", "2026-09-02T00:00:00Z"),
                commit("bbb", "2026-09-04T00:00:00Z"),
            ],
            comments: [comment(1, "2026-09-03T00:00:00Z")],
            events: [],
            reviews: [],
            reviewComments: [],
        });
        expect(timeline.map((item) => item.kind)).toEqual([
            "body",
            "commit",
            "comment",
            "commit",
        ]);
    });

    /** A commit can be backdated or rebased, so a comment on it can land
     * before the pull request was opened. The description still leads. */
    it("keeps the opening body on top when a row predates it", () => {
        const timeline = buildPullTimeline(THREAD, {
            commits: [commit("aaa", "2026-08-30T00:00:00Z")],
            comments: [comment(1, "2026-08-31T00:00:00Z")],
            events: [],
            reviews: [],
            reviewComments: [],
        });
        expect(timeline.map((item) => item.kind)).toEqual([
            "body",
            "commit",
            "comment",
        ]);
    });

    it("gives every row a distinct key", () => {
        const timeline = buildPullTimeline(THREAD, {
            commits: [commit("abc123", "2026-09-03T00:00:00Z")],
            comments: [comment(1, "2026-09-02T00:00:00Z")],
            events: [event("labeled", "2026-09-05T00:00:00Z")],
            reviews: [review(1, "2026-09-03T00:00:00Z")],
            reviewComments: [reviewComment(1, "2026-09-04T00:00:00Z")],
        });
        const keys = timeline.map(timelineKey);
        // A comment, a review, and an inline comment can share the same id.
        expect(new Set(keys).size).toBe(keys.length);
    });
});

describe("isTimelineMessage", () => {
    it("treats the body and comments as stackable messages", () => {
        const timeline = buildPullTimeline(THREAD, {
            commits: [commit("abc123", "2026-09-03T00:00:00Z")],
            comments: [comment(1, "2026-09-02T00:00:00Z")],
            events: [event("labeled", "2026-09-03T00:00:00Z")],
            reviews: [review(2, "2026-09-04T00:00:00Z")],
            reviewComments: [],
        });
        expect(timeline.filter(isTimelineMessage)).toHaveLength(2);
    });
});

describe("timelineParticipants", () => {
    it("merges the backend list with comment authors, deduped by login", () => {
        const merged = timelineParticipants(
            [user("Octocat"), user("hubot")],
            [{ author: user("hubot") }, { author: user("Ada") }]
        );
        expect(merged.map((u) => u.login)).toEqual(["Octocat", "hubot", "Ada"]);
    });

    it("ignores blank logins", () => {
        expect(timelineParticipants([user("")], [])).toEqual([]);
    });
});
