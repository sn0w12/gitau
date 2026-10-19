import { useSelector } from "@tanstack/react-store";
import { useCallback } from "react";

import type {
    SelectOption,
    SettingDefinition,
    SettingsSchema,
} from "@/lib/backend/protocol";
import {
    findSettingDefinition,
    getSettingOptions,
} from "@/lib/settings/schema";
import type {
    SettingKey,
    SettingsValues,
} from "@/lib/settings/settings.generated";
import { getSetting, setSetting, settingsStore } from "@/stores/settings-store";

/**
 * Typed reactive read/write for one setting: writes apply synchronously to
 * the store; persistence is coalesced and serialized in the background.
 */
export function useSetting<TKey extends SettingKey>(
    key: TKey
): {
    value: SettingsValues[TKey];
    setValue: (value: SettingsValues[TKey]) => void;
} {
    const value = useSettingValue(key);
    const setValue = useCallback(
        (next: SettingsValues[TKey]) => setSetting(key, next),
        [key]
    );

    return { value, setValue };
}

/** Reactive read of one setting value; throws before initialization. */
export function useSettingValue<TKey extends SettingKey>(
    key: TKey
): SettingsValues[TKey] {
    return useSelector(settingsStore, (state) => {
        if (!state.ready) {
            throw new Error(
                `setting \`${key}\` read before settings were initialized`
            );
        }
        return state.values[key];
    });
}

export function useSettingsSchema(): SettingsSchema {
    return useSelector(settingsStore, (state) => state.schema);
}

/** The schema entry for one setting, or undefined before/without it. */
export function useSettingDefinition<TKey extends SettingKey>(
    key: TKey
): SettingDefinition | undefined {
    return useSelector(settingsStore, (state) =>
        findSettingDefinition(state.schema, key)
    );
}

export function useSettingsValues(): SettingsValues {
    return useSelector(settingsStore, (state) => {
        if (!state.ready) {
            throw new Error("settings read before settings were initialized");
        }
        return state.values;
    });
}

/** A select setting's options, straight from the schema. */
export function useSettingOptions<TKey extends SettingKey>(
    key: TKey
): SelectOption[] {
    return useSelector(settingsStore, (state) =>
        getSettingOptions(state.schema, key)
    );
}

export { getSetting };
