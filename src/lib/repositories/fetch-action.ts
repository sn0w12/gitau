import type { BranchInfo, HeadState, RemoteInfo } from "@/lib/backend/protocol";

/**
 * The toolbar sync button's resolved state, mirroring GitHub Desktop's
 * PushPullButton render order: publish repo without a remote, fetch for an
 * unborn or in-sync branch, a disabled publish for detached HEAD, publish
 * without an upstream, and pull (behind, including diverged) before push
 * (ahead only).
 */
export type FetchAction =
    | { kind: "noRemote" }
    | { kind: "fetch" }
    | { kind: "detachedHead" }
    | { kind: "publishBranch" }
    | { kind: "pull"; ahead: number; behind: number }
    | { kind: "push"; ahead: number; behind: number };

/**
 * Resolves what the toolbar button should do, 1:1 with GitHub Desktop. Like
 * its renderButton, the in-sync case wins before the behind/ahead split, and
 * a diverged branch shows a pull (behind) button, never a push one.
 */
export function deriveFetchAction(
    head: HeadState,
    branches: BranchInfo[],
    remotes: RemoteInfo[]
): FetchAction {
    if (remotes.length === 0) return { kind: "noRemote" };
    if (head.state === "detached") return { kind: "detachedHead" };
    if (head.state === "unborn") return { kind: "fetch" };

    const current = findCurrentBranch(head, branches);
    // Attached but no branch record yet is safe to fetch.
    if (!current) return { kind: "fetch" };
    if (!current.upstream) return { kind: "publishBranch" };

    const { ahead, behind } = current.upstream;
    if (ahead === 0 && behind === 0) return { kind: "fetch" };
    if (behind > 0) return { kind: "pull", ahead, behind };
    return { kind: "push", ahead, behind };
}

export function findCurrentBranch(
    head: HeadState,
    branches: BranchInfo[]
): BranchInfo | undefined {
    if (head.state !== "attached") return undefined;
    return branches.find((branch) => branch.isHead);
}

export function primaryRemoteName(remotes: RemoteInfo[]): string | undefined {
    return (
        remotes.find((remote) => remote.name === "origin")?.name ??
        remotes[0]?.name
    );
}
