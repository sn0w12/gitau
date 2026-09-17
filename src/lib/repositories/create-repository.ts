import type { CreateRepositoryRequest } from "@/lib/backend/protocol";
import type { BackendClient } from "@/lib/backend/transport/client";
import type { GitBackendError } from "@/lib/backend/transport/invoke";
import { ensureRepo, registerOpen } from "@/stores/repository-store";

export interface CreatedRepository {
    repoId: number;
    repoPath: string;
}

export type CreateRepositoryOutcome =
    | { status: "created"; repo: CreatedRepository }
    | { status: "failed"; error: GitBackendError };

/** Composes the target path the same way the dialog previews it. */
export function joinRepoPath(parentDirectory: string, name: string): string {
    const trimmedParent = parentDirectory.replace(/[\\/]+$/, "");
    return `${trimmedParent}/${name.trim()}`;
}

export async function createRepositoryOnDisk(
    backend: BackendClient,
    request: CreateRepositoryRequest
): Promise<CreateRepositoryOutcome> {
    const result = await backend.repositories.create(request);
    if (!result.ok) {
        return { status: "failed", error: result.error };
    }
    ensureRepo(result.value.path);
    registerOpen(result.value.path, result.value.id);
    return {
        status: "created",
        repo: { repoId: result.value.id, repoPath: result.value.path },
    };
}
