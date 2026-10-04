import { useCallback } from "react";

import { useAppServices } from "@/contexts/services-context";
import { toastError } from "@/lib/toast-error";

/** Opens the system default terminal in the given directory; failures
 * surface as a toast. */
export function useOpenInTerminal(): (path: string) => Promise<void> {
    const { backend } = useAppServices();

    return useCallback(
        async (path: string) => {
            const result = await backend.terminal.open(path);
            if (!result.ok) {
                toastError("Could not open in terminal", result.error);
            }
        },
        [backend]
    );
}
