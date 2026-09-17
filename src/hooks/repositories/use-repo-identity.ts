import { useQueries } from "@tanstack/react-query";

import { useAppServices } from "@/contexts/services-context";
import { remotesQuery } from "@/lib/backend/queries/repository-queries";

import { useRemotes, useRemotesByPath } from "./use-repository-queries";

export interface RepoIdentity {
    name: string;
    owner: string | undefined;
    initial: string;
}

export function repoDisplayName(repoPath: string): string {
    const trimmed = repoPath.replace(/[\\/]+$/, "");
    const segment = trimmed.split(/[\\/]/).pop() ?? "";
    return segment || "repo";
}

export function ownerFromRemote(
    remote: string | undefined
): string | undefined {
    if (!remote) return undefined;

    const withoutSuffix = remote.trim().replace(/\.git$/, "");

    if (withoutSuffix.includes("://")) {
        const afterScheme = withoutSuffix.split("://")[1] ?? "";
        const hostEnd = afterScheme.indexOf("/");
        if (hostEnd === -1) return undefined;
        return afterScheme.slice(hostEnd + 1).split("/")[0] || undefined;
    }

    // scp-like: git@host:owner/repo
    if (withoutSuffix.includes("@") && withoutSuffix.includes(":")) {
        const afterColon = withoutSuffix.split(":").pop() ?? "";
        return afterColon.split("/")[0] || undefined;
    }

    return undefined;
}

export function formatRepoLabel(identity: RepoIdentity): string {
    return identity.owner
        ? `${identity.owner}/${identity.name}`
        : identity.name;
}

export function useRepoIdentity(
    repoId: number | undefined,
    repoPath: string
): RepoIdentity {
    const remotes = useRemotes(repoId);
    return identityFromRemotes(repoPath, remotes.data);
}

export function useRepoIdentityByPath(repoPath: string): RepoIdentity {
    const remotes = useRemotesByPath(repoPath);
    return identityFromRemotes(repoPath, remotes.data);
}

function originUrl(
    remotes: ReadonlyArray<{ name: string; url?: string }> | undefined
): string | undefined {
    return (
        remotes?.find((remote) => remote.name === "origin")?.url ??
        remotes?.[0]?.url
    );
}

function identityFromRemotes(
    repoPath: string,
    remotes: ReadonlyArray<{ name: string; url?: string }> | undefined
): RepoIdentity {
    const name = repoDisplayName(repoPath);
    const owner = ownerFromRemote(originUrl(remotes));
    return {
        name,
        owner,
        initial: (name.charAt(0) || "R").toUpperCase(),
    };
}

export interface RepoOwnerInput {
    repoId: number;
    path: string;
}

// Per-repo hooks cannot be called from a map during render (hook-count
// violation), so lists aggregate identity lookups through one useQueries call.
export function useRepoIdentities(
    repos: ReadonlyArray<RepoOwnerInput>
): Map<string, string | undefined> {
    const { backend } = useAppServices();
    const results = useQueries({
        queries: repos.map((repo) => ({
            ...remotesQuery({ backend }, repo.repoId),
            enabled: Number.isInteger(repo.repoId) && repo.repoId > 0,
        })),
    });

    const owners = new Map<string, string | undefined>();
    for (const [index, repo] of repos.entries()) {
        const entry = results[index]?.data;
        owners.set(repo.path, ownerFromRemote(originUrl(entry)));
    }
    return owners;
}
