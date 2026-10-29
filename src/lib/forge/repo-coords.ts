import type { ProviderDescriptor, RemoteInfo } from "@/lib/backend/protocol";

export interface RepoCoords {
    owner: string;
    repo: string;
}

/** Repository coordinates resolved to the provider that owns the host. */
export interface RemoteCoords extends RepoCoords {
    provider: string;
    host: string;
}

const BARE_PATTERN = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/;

interface HostPath {
    host: string;
    owner: string;
    repo: string;
}

/**
 * Splits a remote URL into host and `owner/repo`. Handles
 * `scheme://[user@]host[:port]/path` and scp-style `user@host:path`, which
 * covers every shape a git remote uses. Hosts are compared lowercased with
 * a leading `www.` removed.
 */
function hostAndPath(url: string): HostPath | null {
    const match = url.match(
        /^(?:[a-z][a-z0-9+.-]*:\/\/)?(?:[^@/]+@)?([^/:]+)(?::\d+)?[/:](.+)$/i
    );
    if (!match) return null;
    const host = match[1].replace(/^www\./i, "").toLowerCase();
    const path = match[2].replace(/\.git\/?$/i, "").replace(/\/+$/, "");
    const segments = path.split("/").filter(Boolean);
    if (segments.length < 2) return null;
    return {
        host,
        owner: segments[segments.length - 2],
        repo: segments[segments.length - 1],
    };
}

function resolveProvider(
    host: string,
    providers: ProviderDescriptor[]
): ProviderDescriptor | null {
    const lower = host.toLowerCase();
    return (
        providers.find((provider) =>
            provider.hosts.some(
                (candidate) => candidate.toLowerCase() === lower
            )
        ) ?? null
    );
}

/**
 * Resolves a remote URL to provider coordinates. Accepts HTTPS, HTTP, SSH,
 * scp-style, and bare `owner/repo` (matched against the first provider's
 * primary host). Null when no provider claims the host.
 */
export function parseRemoteCoords(
    url: string | undefined,
    providers: ProviderDescriptor[]
): RemoteCoords | null {
    if (!url) return null;
    const trimmed = url.trim();
    if (!trimmed) return null;

    const parsed = BARE_PATTERN.test(trimmed)
        ? bareCoords(trimmed, providers)
        : hostAndPath(trimmed);
    if (!parsed) return null;

    const provider =
        resolveProvider(parsed.host, providers) ??
        (BARE_PATTERN.test(trimmed) ? providers[0] : undefined);
    if (!provider) return null;
    return {
        provider: provider.id,
        host: parsed.host || provider.hosts[0] || "",
        owner: parsed.owner,
        repo: parsed.repo,
    };
}

function bareCoords(
    trimmed: string,
    providers: ProviderDescriptor[]
): HostPath | null {
    const [owner, repo] = trimmed.split("/");
    if (!owner || !repo) return null;
    const host = providers[0]?.hosts[0] ?? "";
    return { host, owner, repo };
}

/**
 * Picks coordinates for a repo from its remotes, preferring `origin`. Null
 * when no remote is claimed by a known provider.
 */
export function pickRepoCoords(
    remotes: RemoteInfo[],
    providers: ProviderDescriptor[]
): RemoteCoords | null {
    const remote = pickRepoRemote(remotes, providers);
    return remote ? parseRemoteCoords(remote.url, providers) : null;
}

/**
 * The remote [`pickRepoCoords`] resolves, for operations that must name their
 * remote instead of relying on the backend's `origin` default.
 */
export function pickRepoRemote(
    remotes: RemoteInfo[],
    providers: ProviderDescriptor[]
): RemoteInfo | null {
    const ordered = [...remotes].sort((a, b) =>
        a.name === "origin" ? -1 : b.name === "origin" ? 1 : 0
    );
    return (
        ordered.find((remote) => parseRemoteCoords(remote.url, providers)) ??
        null
    );
}

export interface ParsedThreadUrl extends RepoCoords {
    number: number;
}

export type ParsedIssueUrl = ParsedThreadUrl;

const ISSUE_WEB_PATTERN =
    /^https?:\/\/[^/]+\/([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)\/issues\/(\d+)/i;
const ISSUE_API_PATTERN =
    /^https?:\/\/api\.[^/]+\/repos\/([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)\/issues\/(\d+)/i;
const PULL_WEB_PATTERN =
    /^https?:\/\/[^/]+\/([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)\/pull\/(\d+)/i;
const PULL_API_PATTERN =
    /^https?:\/\/api\.[^/]+\/repos\/([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)\/pulls\/(\d+)/i;

/** Parses an issue web URL or API subject URL into `owner/repo` and number. */
export function parseIssueUrl(url: string | undefined): ParsedIssueUrl | null {
    if (!url) return null;
    const match = url.match(ISSUE_WEB_PATTERN) ?? url.match(ISSUE_API_PATTERN);
    if (!match) return null;
    return { owner: match[1], repo: match[2], number: Number(match[3]) };
}

/** The pull request counterpart of [`parseIssueUrl`]. */
export function parsePullUrl(url: string | undefined): ParsedThreadUrl | null {
    if (!url) return null;
    const match = url.match(PULL_WEB_PATTERN) ?? url.match(PULL_API_PATTERN);
    if (!match) return null;
    return { owner: match[1], repo: match[2], number: Number(match[3]) };
}
