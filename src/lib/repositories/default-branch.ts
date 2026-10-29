/** Conventional defaults until the backend exposes the remote HEAD symref. */
export const DEFAULT_BRANCH_NAMES = ["main", "master"];

export function isDefaultBranchName(name: string): boolean {
    return DEFAULT_BRANCH_NAMES.includes(name.toLowerCase());
}

/**
 * The base a new pull request starts on: a conventional default branch when
 * one exists locally, otherwise the first branch that is not `head`.
 */
export function pickBaseBranch(names: readonly string[], head: string): string {
    const candidates = names.filter((name) => name !== head);
    for (const name of DEFAULT_BRANCH_NAMES) {
        const match = candidates.find(
            (candidate) => candidate.toLowerCase() === name
        );
        if (match) return match;
    }
    return candidates[0] ?? "";
}
