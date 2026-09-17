import { useCallback } from "react";

import { useAppServices } from "@/contexts/services-context";
import { useSettingValue } from "@/hooks/settings/use-setting";
import { toastError } from "@/lib/toast-error";

/**
 * Launches the editor configured in `editorCommand` with the given target.
 * `enabled` stays false until the user sets a command, so menu items can
 * disable themselves; launch failures surface as a toast.
 */
export function useOpenInEditor(): {
    enabled: boolean;
    openInEditor: (path: string, relativePath?: string) => Promise<void>;
} {
    const { backend } = useAppServices();
    const editorCommand = useSettingValue("editorCommand");

    const openInEditor = useCallback(
        async (path: string, relativePath?: string) => {
            const result = await backend.editor.openInEditor(
                path,
                relativePath
            );
            if (!result.ok) {
                toastError("Could not open in editor", result.error);
            }
        },
        [backend]
    );

    // Test seeds may omit the key; a missing command means not configured.
    return {
        enabled: Boolean(editorCommand) && editorCommand.trim().length > 0,
        openInEditor,
    };
}
