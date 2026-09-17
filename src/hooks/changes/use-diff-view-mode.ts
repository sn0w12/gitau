import { useSelector } from "@tanstack/react-store";

import type { SettingsValues } from "@/lib/settings/settings.generated";
import { setSetting, settingsStore } from "@/stores/settings-store";

export type DiffViewMode = SettingsValues["diffViewMode"];

export function useDiffViewMode(): {
    mode: DiffViewMode;
    setMode: (mode: DiffViewMode) => void;
} {
    const mode = useSelector(settingsStore, (state) =>
        state.ready ? state.values.diffViewMode : "unified"
    );
    return { mode, setMode: setSetting.bind(null, "diffViewMode") };
}
