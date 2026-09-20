import { describe, expect, it } from "vitest";

import type { RemoteInfo } from "@/lib/backend/protocol";
import {
    parseGithubCoords,
    parseIssueUrl,
    pickGithubCoords,
} from "@/lib/github/repo-coords";

function remote(name: string, url: string): RemoteInfo {
    return { name, url };
}

describe("parseGithubCoords", () => {
    it("parses https, ssh, and bare forms", () => {
        expect(parseGithubCoords("https://github.com/octo/repo.git")).toEqual({
            owner: "octo",
            repo: "repo",
        });
        expect(parseGithubCoords("https://github.com/octo/repo")).toEqual({
            owner: "octo",
            repo: "repo",
        });
        expect(parseGithubCoords("git@github.com:octo/repo.git")).toEqual({
            owner: "octo",
            repo: "repo",
        });
        expect(parseGithubCoords("octo/repo")).toEqual({
            owner: "octo",
            repo: "repo",
        });
    });

    it("rejects non-github urls", () => {
        expect(
            parseGithubCoords("https://gitlab.com/octo/repo.git")
        ).toBeNull();
        expect(
            parseGithubCoords("https://github.evil.dev/octo/repo")
        ).toBeNull();
        expect(parseGithubCoords(undefined)).toBeNull();
    });
});

describe("pickGithubCoords", () => {
    it("prefers origin", () => {
        expect(
            pickGithubCoords([
                remote("upstream", "https://github.com/up/repo.git"),
                remote("origin", "git@github.com:octo/repo.git"),
            ])
        ).toEqual({ owner: "octo", repo: "repo" });
    });

    it("returns null without a github remote", () => {
        expect(
            pickGithubCoords([remote("origin", "https://gitlab.com/o/r.git")])
        ).toBeNull();
    });
});

describe("parseIssueUrl", () => {
    it("parses web and api urls", () => {
        expect(parseIssueUrl("https://github.com/octo/repo/issues/42")).toEqual(
            { owner: "octo", repo: "repo", number: 42 }
        );
        expect(
            parseIssueUrl("https://api.github.com/repos/octo/repo/issues/7")
        ).toEqual({ owner: "octo", repo: "repo", number: 7 });
    });

    it("rejects pull and non-issue urls", () => {
        expect(
            parseIssueUrl("https://github.com/octo/repo/pull/42")
        ).toBeNull();
        expect(parseIssueUrl(undefined)).toBeNull();
    });
});
