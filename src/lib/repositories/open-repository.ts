import { GitBackendError } from "@/lib/backend/transport/invoke";
import { getAppRuntime } from "@/lib/bootstrap/app-runtime";
import {
    ensureRepo,
    getEntryByPath,
    registerOpen,
    setEntryError,
} from "@/stores/repository-store";

export type OpenRepositoryResult =
    | { status: "opened"; repoId: number }
    | { status: "failed"; error: GitBackendError };

/**
 * Ensures a repository is open in the backend and returns its
 * process-local id. Idempotent; a failed attempt records the error on the
 * store entry and can be retried by calling again.
 */
export async function openRepositoryByPath(
    path: string
): Promise<OpenRepositoryResult> {
    if (!path || typeof path !== "string") {
        // A protocol slip upstream must land in the visible toast path, not
        // crash inside store string handling and vanish into a void call.
        return {
            status: "failed",
            error: new GitBackendError({
                code: "invalidInput",
                message: `cannot open repository: invalid path`,
                detail: undefined,
                retryable: false,
            }),
        };
    }
    const existing = getEntryByPath(path)?.repoId;
    if (existing !== undefined) {
        return { status: "opened", repoId: existing };
    }

    ensureRepo(path);
    const opened = await getAppRuntime().backend.repositories.open(path);
    if (!opened.ok) {
        setEntryError(path, opened.error.message);
        return { status: "failed", error: opened.error };
    }
    registerOpen(path, opened.value.id);
    return { status: "opened", repoId: opened.value.id };
}
