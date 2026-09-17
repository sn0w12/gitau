import { useQuery } from "@tanstack/react-query";

import { useAppServices } from "@/contexts/services-context";
import {
    repoIconByPathQuery,
    repoIconQuery,
} from "@/lib/backend/queries/repository-queries";

/**
 * Icon for one repository: the backend prefers an icon file found in the
 * worktree and falls back to the owner avatar of its origin remote.
 */
export function useRepoIcon(repoId: number | undefined) {
    const { backend } = useAppServices();
    return useQuery({
        ...repoIconQuery({ backend }, repoId ?? 0),
        enabled:
            typeof repoId === "number" &&
            Number.isInteger(repoId) &&
            repoId > 0,
    });
}

/** Icon for a repository addressed by path, independent of any open tab. */
export function useRepoIconByPath(path: string | undefined) {
    const { backend } = useAppServices();
    return useQuery({
        ...repoIconByPathQuery({ backend }, path ?? ""),
        enabled: !!path,
    });
}
