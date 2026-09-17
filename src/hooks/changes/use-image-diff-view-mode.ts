import { useSelector } from "@tanstack/react-store";

import type { SettingsValues } from "@/lib/settings/settings.generated";
import { setSetting, settingsStore } from "@/stores/settings-store";

export type ImageDiffViewMode = SettingsValues["imageDiffViewMode"];

export function useImageDiffViewMode(): {
    mode: ImageDiffViewMode;
    setMode: (mode: ImageDiffViewMode) => void;
} {
    const mode = useSelector(settingsStore, (state) =>
        state.ready ? state.values.imageDiffViewMode : "sideBySide"
    );
    return {
        mode,
        setMode: setSetting.bind(null, "imageDiffViewMode"),
    };
}
