// @vitest-environment jsdom
import { describe, expect, it } from "vitest";

import {
    isOpenThread,
    issueStatusOf,
    pullRequestStatusOf,
} from "@/components/github/status-badge";

describe("issueStatusOf", () => {
    it("reads the close reason, which state alone does not carry", () => {
        expect(
            issueStatusOf({ state: "closed", stateReason: "completed" })
        ).toBe("completed");
        expect(
            issueStatusOf({ state: "closed", stateReason: "notPlanned" })
        ).toBe("notPlanned");
        expect(
            issueStatusOf({ state: "closed", stateReason: "duplicate" })
        ).toBe("duplicate");
        expect(issueStatusOf({ state: "closed", stateReason: "none" })).toBe(
            "closed"
        );
    });

    /** An open issue still carries the reason it was reopened with, so the
     * reason must not turn an open thread into a closed status. */
    it("keeps a reopened issue open", () => {
        expect(
            issueStatusOf({ state: "open", stateReason: "notPlanned" })
        ).toBe("open");
        expect(issueStatusOf({ state: "OPEN" })).toBe("open");
    });

    it("tolerates a missing reason", () => {
        expect(issueStatusOf({ state: "closed" })).toBe("closed");
    });
});

describe("pullRequestStatusOf", () => {
    it("outranks the reason with the merge timestamp and the draft flag", () => {
        expect(
            pullRequestStatusOf({
                state: "closed",
                stateReason: "notPlanned",
                mergedAt: "2026-09-02T09:00:00Z",
            })
        ).toBe("merged");
        expect(
            pullRequestStatusOf({
                state: "open",
                draft: true,
                stateReason: "none",
            })
        ).toBe("draft");
    });

    it("falls through to the reason for a closed pull request", () => {
        expect(
            pullRequestStatusOf({
                state: "closed",
                stateReason: "notPlanned",
                mergedAt: null,
            })
        ).toBe("notPlanned");
    });
});

describe("isOpenThread", () => {
    it("answers from the state alone", () => {
        expect(isOpenThread({ state: "open" })).toBe(true);
        expect(isOpenThread({ state: "closed" })).toBe(false);
    });
});
