import { useSelector } from "@tanstack/react-store";

import { repositoryStore } from "@/stores/repository-store";

/** Durable repo path for an ephemeral backend repoId, or "" when the repo
 * is not registered in this session. */
export function useRepoPath(repoId: number | undefined): string {
    return useSelector(repositoryStore, (state) => {
        if (repoId === undefined) return "";
        for (const entry of state.entries.values()) {
            if (entry.repoId === repoId) return entry.path;
        }
        return "";
    });
}
