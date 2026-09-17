import type { CachedIcon } from "@/lib/backend/protocol";

import { useRepoIcon, useRepoIconByPath } from "./use-repo-icon";
import { useRepoIdentity, useRepoIdentityByPath } from "./use-repo-identity";

export interface RepoAvatarInfo {
    name: string;
    owner: string | undefined;
    initial: string;
    icon: CachedIcon | null | undefined;
}

/** Composition of the identity hook (owner/name) and the repo icon hook. */
export function useRepoAvatar(
    repoId: number | undefined,
    repoPath: string
): RepoAvatarInfo {
    const identity = useRepoIdentity(repoId, repoPath);
    const icon = useRepoIcon(repoId);

    return {
        name: identity.name,
        owner: identity.owner,
        initial: identity.initial,
        icon: icon.data,
    };
}

/**
 * Like `useRepoAvatar` but addressed by the durable path, so icons and
 * owner labels survive closing a repo's last tab.
 */
export function useRepoAvatarByPath(repoPath: string): RepoAvatarInfo {
    const identity = useRepoIdentityByPath(repoPath);
    const icon = useRepoIconByPath(repoPath);

    return {
        name: identity.name,
        owner: identity.owner,
        initial: identity.initial,
        icon: icon.data,
    };
}
