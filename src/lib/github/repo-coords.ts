import type { RemoteInfo } from "@/lib/backend/protocol";

export interface GithubCoords {
    owner: string;
    repo: string;
}

const COORDS_PATTERN = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/;

/**
 * Parses `owner/repo` out of a GitHub remote URL. Accepts HTTPS
 * (`https://github.com/o/r(.git)`), SSH (`git@github.com:o/r.git`,
 * `ssh://git@github.com/o/r(.git)`), and bare `o/r`. Anything else
 * yields null.
 */
export function parseGithubCoords(
    url: string | undefined
): GithubCoords | null {
    if (!url) return null;
    const trimmed = url.trim();
    if (COORDS_PATTERN.test(trimmed)) {
        const [owner, repo] = trimmed.split("/");
        return { owner, repo };
    }
    let path: string | null = null;
    const https = trimmed.match(/^https?:\/\/github\.com[/:](.+)$/i);
    if (https) {
        path = https[1];
    } else {
        const ssh = trimmed.match(/^(?:[^@]+@)?github\.com:(.+)$/i);
        if (ssh) {
            path = ssh[1];
        } else {
            const sshUrl = trimmed.match(
                /^ssh:\/\/(?:[^@]+@)?github\.com[/:](.+)$/i
            );
            if (sshUrl) path = sshUrl[1];
        }
    }
    if (!path) return null;
    const segments = path.replace(/\.git\/?$/, "").split("/");
    if (segments.length < 2) return null;
    const owner = segments[segments.length - 2];
    const repo = segments[segments.length - 1];
    if (!owner || !repo) return null;
    return { owner, repo };
}

/**
 * Picks the GitHub coordinates for a repo from its remotes, preferring
 * `origin`. Null when no remote points at github.com.
 */
export function pickGithubCoords(remotes: RemoteInfo[]): GithubCoords | null {
    const ordered = [...remotes].sort((a, b) =>
        a.name === "origin" ? -1 : b.name === "origin" ? 1 : 0
    );
    for (const remote of ordered) {
        const coords = parseGithubCoords(remote.url);
        if (coords) return coords;
    }
    return null;
}

export interface ParsedIssueUrl extends GithubCoords {
    number: number;
}

/**
 * Parses a GitHub issue web URL or API subject URL into coordinates plus
 * the issue number. Web: `https://github.com/o/r/issues/N`. API:
 * `https://api.github.com/repos/o/r/issues/N`.
 */
export function parseIssueUrl(url: string | undefined): ParsedIssueUrl | null {
    if (!url) return null;
    const web = url.match(
        /^https?:\/\/github\.com\/([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)\/issues\/(\d+)/i
    );
    if (web) {
        return { owner: web[1], repo: web[2], number: Number(web[3]) };
    }
    const api = url.match(
        /^https?:\/\/api\.github\.com\/repos\/([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)\/issues\/(\d+)/i
    );
    if (api) {
        return { owner: api[1], repo: api[2], number: Number(api[3]) };
    }
    return null;
}
