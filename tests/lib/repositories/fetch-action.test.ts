import { describe, expect, it } from "vitest";

import type { BranchInfo, HeadState, RemoteInfo } from "@/lib/backend/protocol";
import {
    deriveFetchAction,
    primaryRemoteName,
} from "@/lib/repositories/fetch-action";

function branch(overrides: Partial<BranchInfo> = {}): BranchInfo {
    return {
        name: "main",
        target: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b",
        isHead: false,
        ...overrides,
    };
}

const attached: HeadState = {
    state: "attached",
    branch: "main",
    target: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b",
};

const origin: RemoteInfo = { name: "origin", url: "https://x.test/a/b" };

describe("deriveFetchAction", () => {
    it("is disabled when the repository has no remotes", () => {
        expect(
            deriveFetchAction(attached, [branch({ isHead: true })], [])
        ).toEqual({ kind: "noRemote" });
    });

    it("offers to publish a branch without upstream tracking", () => {
        expect(
            deriveFetchAction(attached, [branch({ isHead: true })], [origin])
        ).toEqual({ kind: "publishBranch" });
    });

    it("fetches when the current branch has no ahead/behind data yet", () => {
        expect(
            deriveFetchAction(
                attached,
                [
                    branch({
                        isHead: true,
                        upstream: {
                            remote: "origin",
                            branch: "main",
                            ahead: 0,
                            behind: 0,
                        },
                    }),
                ],
                [origin]
            )
        ).toEqual({ kind: "fetch" });
    });

    it("pulls when strictly behind", () => {
        expect(
            deriveFetchAction(
                attached,
                [
                    branch({
                        isHead: true,
                        upstream: {
                            remote: "origin",
                            branch: "main",
                            ahead: 0,
                            behind: 3,
                        },
                    }),
                ],
                [origin]
            )
        ).toEqual({ kind: "pull", ahead: 0, behind: 3 });
    });

    it("pushes when strictly ahead", () => {
        expect(
            deriveFetchAction(
                attached,
                [
                    branch({
                        isHead: true,
                        upstream: {
                            remote: "origin",
                            branch: "main",
                            ahead: 2,
                            behind: 0,
                        },
                    }),
                ],
                [origin]
            )
        ).toEqual({ kind: "push", ahead: 2, behind: 0 });
    });

    it("pulls first when diverged (matches GitHub Desktop, no push state)", () => {
        expect(
            deriveFetchAction(
                attached,
                [
                    branch({
                        isHead: true,
                        upstream: {
                            remote: "origin",
                            branch: "main",
                            ahead: 1,
                            behind: 4,
                        },
                    }),
                ],
                [origin]
            )
        ).toEqual({ kind: "pull", ahead: 1, behind: 4 });
    });

    it("disables publish for detached HEAD with remotes present", () => {
        const detached: HeadState = {
            state: "detached",
            target: "1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b",
        };
        expect(deriveFetchAction(detached, [], [origin])).toEqual({
            kind: "detachedHead",
        });
    });

    it("fetches for an unborn branch", () => {
        const unborn: HeadState = { state: "unborn", branch: "main" };
        expect(deriveFetchAction(unborn, [], [origin])).toEqual({
            kind: "fetch",
        });
    });
});

describe("primaryRemoteName", () => {
    it("prefers origin over other remotes", () => {
        expect(
            primaryRemoteName([{ name: "upstream" }, { name: "origin" }])
        ).toBe("origin");
    });

    it("falls back to the first listed remote", () => {
        expect(primaryRemoteName([{ name: "fork" }])).toBe("fork");
    });

    it("is undefined without remotes", () => {
        expect(primaryRemoteName([])).toBeUndefined();
    });
});
