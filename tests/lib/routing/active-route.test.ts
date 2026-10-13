import { describe, expect, it } from "vitest";

import { isRepoPathname, pathnameOf } from "@/lib/routing/active-route";

describe("pathnameOf", () => {
    it("drops the query string and hash", () => {
        expect(pathnameOf("/repo/5?view=graph&commit=abc")).toBe("/repo/5");
        expect(pathnameOf("/inbox#top")).toBe("/inbox");
        expect(pathnameOf("/issues")).toBe("/issues");
    });
});

describe("isRepoPathname", () => {
    it("matches the repo page and its nested routes", () => {
        expect(isRepoPathname("/repo/5")).toBe(true);
        expect(isRepoPathname("/repo/5/issue/42")).toBe(true);
        expect(isRepoPathname("/repo/5/pull/7")).toBe(true);
    });

    it("rejects unrelated and prefix-sharing paths", () => {
        expect(isRepoPathname("/")).toBe(false);
        expect(isRepoPathname("/inbox")).toBe(false);
        expect(isRepoPathname("/repositories")).toBe(false);
    });
});
