import { describe, expect, it } from "vitest";

import {
    infiniteRepoIssuesQuery,
    issueCommentsQuery,
    issueDetailQuery,
    issueEventsQuery,
} from "@/lib/backend/queries/forge-issues-queries";
import {
    infiniteRepoPullsQuery,
    pullRequestQuery,
} from "@/lib/backend/queries/forge-pulls-queries";
import { THREAD_POLL } from "@/lib/backend/queries/forge-queries";
import type { BackendClient } from "@/lib/backend/transport/client";

const backend = {} as BackendClient;

function intervalOf(
    option: unknown,
    data: unknown,
    dataUpdateCount = 1
): number | false {
    if (typeof option !== "function") {
        throw new Error("expected a refetch interval function");
    }
    const fn = option as (query: {
        state: { data: unknown; dataUpdateCount: number };
    }) => number | false | undefined;
    return fn({ state: { data, dataUpdateCount } }) ?? false;
}

describe("forge thread polling", () => {
    it("re-reads a pull request quickly until its merge verdict lands", () => {
        const options = pullRequestQuery(
            { backend },
            "github",
            "owner",
            "repo",
            7,
            true
        );
        expect(intervalOf(options.refetchInterval, { mergeable: null })).toBe(
            10_000
        );
        expect(
            intervalOf(options.refetchInterval, { mergeable: null }, 7)
        ).toBe(THREAD_POLL.refetchInterval);
        expect(intervalOf(options.refetchInterval, undefined)).toBe(
            THREAD_POLL.refetchInterval
        );
        expect(intervalOf(options.refetchInterval, { mergeable: true })).toBe(
            THREAD_POLL.refetchInterval
        );
    });

    it("refreshes issue threads on the steady cadence and on focus", () => {
        const detail = issueDetailQuery(
            { backend },
            "github",
            "owner",
            "repo",
            3,
            true
        );
        const comments = issueCommentsQuery(
            { backend },
            "github",
            "owner",
            "repo",
            3,
            true
        );
        const events = issueEventsQuery(
            { backend },
            "github",
            "owner",
            "repo",
            3,
            true
        );
        for (const query of [detail, comments, events]) {
            expect(query.refetchInterval).toBe(THREAD_POLL.refetchInterval);
            expect(query.refetchOnWindowFocus).toBe(true);
        }
    });

    it("refreshes the repository lists", () => {
        expect(
            infiniteRepoIssuesQuery(
                { backend },
                "github",
                "owner",
                "repo",
                "open",
                [],
                true
            ).refetchInterval
        ).toBe(60_000);
        expect(
            infiniteRepoPullsQuery(
                { backend },
                "github",
                "owner",
                "repo",
                "open",
                [],
                true
            ).refetchInterval
        ).toBe(60_000);
    });
});
