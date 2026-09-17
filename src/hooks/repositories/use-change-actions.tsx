import { useConfirm } from "@/contexts/confirm-context";
import {
    useDiscardMutation,
    useStageMutation,
    useUnstageMutation,
} from "@/lib/backend/mutations/repository-mutations";
import type { StatusEntry } from "@/lib/backend/protocol";
import { toastError } from "@/lib/toast-error";

import { useRepositoryStatus } from "./use-repository-queries";

const MAX_LISTED_DISCARD_PATHS = 3;

// Explicit paths also delete untracked files; the backend `all` flag
// spares them (GitHub Desktop parity).
function distinctPaths(entries: StatusEntry[]): string[] {
    return [...new Set(entries.map((entry) => entry.path))];
}

function DiscardDescription({ paths }: { paths: string[] }) {
    const shown = paths.slice(0, MAX_LISTED_DISCARD_PATHS);
    const rest = paths.length - shown.length;
    return (
        <span>
            This cannot be undone.{" "}
            {shown.map((path) => (
                <span key={path} className="block font-mono text-xs">
                    {path}
                </span>
            ))}
            {rest > 0 && (
                <span className="block text-xs text-muted-foreground">
                    and {rest} more…
                </span>
            )}
        </span>
    );
}

/**
 * Stage, unstage, and discard flows shared by the Changes panel, the repo
 * shortcuts, and the command palette. Discard always confirms first;
 * failures surface as toasts.
 */
export function useChangeActions(repoId: number | undefined) {
    const status = useRepositoryStatus(repoId);
    const stage = useStageMutation(repoId ?? 0);
    const unstage = useUnstageMutation(repoId ?? 0);
    const discard = useDiscardMutation(repoId ?? 0);
    const { confirm } = useConfirm();

    const stageEntries = async (entries: StatusEntry[]) => {
        const paths = distinctPaths(entries);
        if (paths.length === 0) return;
        try {
            await stage.mutateAsync({ paths });
        } catch (error) {
            toastError("Could not stage changes", error);
        }
    };

    const unstageEntries = async (entries: StatusEntry[]) => {
        const paths = distinctPaths(entries);
        if (paths.length === 0) return;
        try {
            await unstage.mutateAsync({ paths });
        } catch (error) {
            toastError("Could not unstage changes", error);
        }
    };

    const discardEntries = async (entries: StatusEntry[]) => {
        const paths = distinctPaths(entries);
        if (paths.length === 0) return;

        const result = await confirm({
            title:
                paths.length === 1
                    ? "Discard changes?"
                    : `Discard changes in ${paths.length} files?`,
            description: <DiscardDescription paths={paths} />,
            confirmText: "Discard",
            variant: "destructive",
        });
        if (!result.confirmed) return;

        try {
            await discard.mutateAsync({ paths });
        } catch (error) {
            toastError("Could not discard changes", error);
        }
    };

    const entries = status.data?.entries ?? [];
    const stagedEntries = entries.filter((entry) => entry.side === "index");
    const unstagedEntries = entries.filter(
        (entry) => entry.side === "worktree"
    );

    return {
        status,
        stage,
        unstage,
        discard,
        stageEntries,
        unstageEntries,
        discardEntries,
        stagedEntries,
        unstagedEntries,
        stageAll: () => stageEntries(unstagedEntries),
        unstageAll: () => unstageEntries(stagedEntries),
        discardAll: () => discardEntries(unstagedEntries),
    };
}
