import { describe, expect, it } from "vitest";

import { isTrustedImageSource } from "@/lib/forge/image-hosts";

const HOSTS = ["github.com", "githubusercontent.com", "githubapp.com"];

describe("isTrustedImageSource", () => {
    it("accepts a host on the list", () => {
        expect(isTrustedImageSource("https://github.com/u/1.png", HOSTS)).toBe(
            true
        );
    });

    it("accepts a subdomain of a listed host", () => {
        expect(
            isTrustedImageSource(
                "https://dependabot-badges.githubapp.com/badges/x",
                HOSTS
            )
        ).toBe(true);
        expect(
            isTrustedImageSource(
                "https://avatars.githubusercontent.com/u/1",
                HOSTS
            )
        ).toBe(true);
    });

    it("matches the host case-insensitively", () => {
        expect(
            isTrustedImageSource("https://AvatarS.GitHubUserContent.com/u/1", [
                "githubusercontent.com",
            ])
        ).toBe(true);
    });

    it("rejects another host", () => {
        expect(isTrustedImageSource("https://example.com/x.png", HOSTS)).toBe(
            false
        );
    });

    it("rejects a suffix glued onto another host", () => {
        for (const source of [
            "https://notgithub.com/x.png",
            "https://github.com.example.com/x.png",
            "https://evilgithubusercontent.com/x.png",
        ]) {
            expect(isTrustedImageSource(source, HOSTS), source).toBe(false);
        }
    });

    it("rejects a non-https source", () => {
        expect(isTrustedImageSource("http://github.com/x.png", HOSTS)).toBe(
            false
        );
    });

    it("rejects a source that is not a url", () => {
        expect(isTrustedImageSource("/images/x.png", HOSTS)).toBe(false);
    });

    it("rejects every source when no host is trusted", () => {
        expect(isTrustedImageSource("https://github.com/x.png", [])).toBe(
            false
        );
    });
});
