import { QueryClient } from "@tanstack/query-core";
import { describe, expect, it, vi } from "vitest";

import { invalidateForRepositoryEvent } from "@/lib/backend/mutations/invalidation";
import { repositoryKeys } from "@/lib/backend/queries/query-keys";

describe("invalidateForRepositoryEvent", () => {
    it("narrows a worktree-only change to status and files", async () => {
        const queryClient = new QueryClient();
        const invalidate = vi.spyOn(queryClient, "invalidateQueries");

        await invalidateForRepositoryEvent(queryClient, 7, "worktree");

        const keys = invalidate.mock.calls.map(
            ([filters]) => filters?.queryKey
        );
        expect(keys).toContainEqual(repositoryKeys.snapshot(7));
        expect(keys).toContainEqual([...repositoryKeys.all(7), "status"]);
        expect(keys).toContainEqual([...repositoryKeys.all(7), "file"]);
        expect(keys).toContainEqual([...repositoryKeys.all(7), "readme"]);
        expect(keys).toContainEqual([...repositoryKeys.all(7), "image"]);
        expect(keys).not.toContainEqual(repositoryKeys.all(7));
        invalidate.mockRestore();
    });

    it("keeps the full sweep for ref changes", async () => {
        const queryClient = new QueryClient();
        const invalidate = vi.spyOn(queryClient, "invalidateQueries");

        await invalidateForRepositoryEvent(queryClient, 7, "refs");

        expect(invalidate).toHaveBeenCalledWith({
            queryKey: repositoryKeys.all(7),
            refetchType: "active",
        });
        invalidate.mockRestore();
    });
});
