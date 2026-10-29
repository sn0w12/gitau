import { describe, expect, it } from "vitest";

import type {
    Capabilities,
    ProviderDescriptor,
    RemoteInfo,
} from "@/lib/backend/protocol";
import {
    parseIssueUrl,
    parsePullUrl,
    parseRemoteCoords,
    pickRepoCoords,
    pickRepoRemote,
} from "@/lib/forge/repo-coords";

const NONE: Capabilities = {
    issues: false,
    pullRequests: false,
    reviews: false,
    notifications: false,
    checks: false,
    commitStatuses: false,
    workflowRuns: false,
    issuesWrite: false,
    pullRequestsWrite: false,
    publish: false,
    orgs: false,
};

function provider(
    id: string,
    label: string,
    hosts: string[]
): ProviderDescriptor {
    return { id, label, hosts, imageHosts: [], capabilities: NONE };
}

const PROVIDERS = [
    provider("github", "GitHub", ["github.com"]),
    provider("gitlab", "GitLab", ["gitlab.com"]),
];

function remote(name: string, url: string): RemoteInfo {
    return { name, url };
}

describe("parseRemoteCoords", () => {
    it("parses https, ssh, scp, and bare forms", () => {
        expect(
            parseRemoteCoords("https://github.com/octo/repo.git", PROVIDERS)
        ).toEqual({
            provider: "github",
            host: "github.com",
            owner: "octo",
            repo: "repo",
        });
        expect(
            parseRemoteCoords("ssh://git@github.com/octo/repo", PROVIDERS)
        ).toEqual({
            provider: "github",
            host: "github.com",
            owner: "octo",
            repo: "repo",
        });
        expect(
            parseRemoteCoords("git@github.com:octo/repo.git", PROVIDERS)
        ).toEqual({
            provider: "github",
            host: "github.com",
            owner: "octo",
            repo: "repo",
        });
        expect(parseRemoteCoords("octo/repo", PROVIDERS)).toEqual({
            provider: "github",
            host: "github.com",
            owner: "octo",
            repo: "repo",
        });
    });

    it("resolves a second provider by host", () => {
        expect(
            parseRemoteCoords("https://gitlab.com/octo/repo.git", PROVIDERS)
        ).toEqual({
            provider: "gitlab",
            host: "gitlab.com",
            owner: "octo",
            repo: "repo",
        });
    });

    it("rejects hosts no provider claims", () => {
        expect(
            parseRemoteCoords("https://github.evil.dev/octo/repo", PROVIDERS)
        ).toBeNull();
        expect(parseRemoteCoords(undefined, PROVIDERS)).toBeNull();
    });
});

describe("pickRepoCoords", () => {
    it("prefers origin", () => {
        expect(
            pickRepoCoords(
                [
                    remote("upstream", "https://github.com/up/repo.git"),
                    remote("origin", "git@github.com:octo/repo.git"),
                ],
                PROVIDERS
            )
        ).toEqual({
            provider: "github",
            host: "github.com",
            owner: "octo",
            repo: "repo",
        });
    });

    it("returns null without a claimed remote", () => {
        expect(
            pickRepoCoords(
                [remote("origin", "https://example.com/o/r.git")],
                PROVIDERS
            )
        ).toBeNull();
    });
});

describe("pickRepoRemote", () => {
    it("names the remote the coordinates came from", () => {
        expect(
            pickRepoRemote(
                [
                    remote("upstream", "https://github.com/up/repo.git"),
                    remote("origin", "git@github.com:octo/repo.git"),
                ],
                PROVIDERS
            )
        ).toEqual(remote("origin", "git@github.com:octo/repo.git"));
    });

    it("falls back to a non-origin remote claimed by a provider", () => {
        expect(
            pickRepoRemote(
                [
                    remote("upstream", "https://example.com/o/r.git"),
                    remote("fork", "https://github.com/octo/repo.git"),
                ],
                PROVIDERS
            )?.name
        ).toBe("fork");
    });

    it("returns null when no remote resolves", () => {
        expect(
            pickRepoRemote(
                [remote("origin", "https://example.com/o/r.git")],
                PROVIDERS
            )
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

describe("parsePullUrl", () => {
    it("parses web and api urls", () => {
        expect(parsePullUrl("https://github.com/octo/repo/pull/42")).toEqual({
            owner: "octo",
            repo: "repo",
            number: 42,
        });
        expect(
            parsePullUrl("https://api.github.com/repos/octo/repo/pulls/7")
        ).toEqual({ owner: "octo", repo: "repo", number: 7 });
    });

    it("rejects issue and non-pull urls", () => {
        expect(
            parsePullUrl("https://github.com/octo/repo/issues/42")
        ).toBeNull();
        expect(parsePullUrl("https://github.com/octo/repo")).toBeNull();
        expect(parsePullUrl(undefined)).toBeNull();
    });
});
