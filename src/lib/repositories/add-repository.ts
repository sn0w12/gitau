import { open as openFileDialog } from "@tauri-apps/plugin-dialog";

import type { GitBackendError } from "@/lib/backend/transport/invoke";

import { openRepositoryByPath } from "./open-repository";

export interface AddedRepository {
    repoId: number;
    repoPath: string;
}

export type AddRepositoryOutcome =
    | { status: "cancelled" }
    | { status: "added"; repo: AddedRepository }
    | { status: "failed"; error: GitBackendError };

// Every terminal state is explicit so callers cannot ignore a failed open.
export async function addExistingRepositoryFromDisk(): Promise<AddRepositoryOutcome> {
    const selection = await openFileDialog({
        directory: true,
        multiple: false,
        title: "Open repository",
    });
    if (typeof selection !== "string" || selection.length === 0) {
        return { status: "cancelled" };
    }

    const opened = await openRepositoryByPath(selection);
    if (opened.status === "failed") {
        return { status: "failed", error: opened.error };
    }

    return {
        status: "added",
        repo: { repoId: opened.repoId, repoPath: selection },
    };
}
