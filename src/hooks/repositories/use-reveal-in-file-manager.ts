import { useCallback } from "react";

import { useAppServices } from "@/contexts/services-context";
import { toastError } from "@/lib/toast-error";

/** Reveals a repo directory or file in the OS file manager; failures
 * surface as a toast. */
export function useRevealInFileManager(): (
    path: string,
    relativePath?: string
) => Promise<void> {
    const { backend } = useAppServices();

    return useCallback(
        async (path: string, relativePath?: string) => {
            const result = await backend.fileManager.reveal(path, relativePath);
            if (!result.ok) {
                toastError("Could not reveal in file manager", result.error);
            }
        },
        [backend]
    );
}
