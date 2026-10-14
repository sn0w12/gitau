import type {
    SelectOption,
    SettingDefinition,
    SettingsSchema,
} from "@/lib/backend/protocol";

export function findSettingDefinition(
    schema: SettingsSchema,
    key: string
): SettingDefinition | undefined {
    for (const tab of schema.tabs) {
        for (const section of tab.sections) {
            for (const setting of section.settings) {
                if (setting.key === key) return setting;
            }
        }
    }
    return undefined;
}

/** Options for one setting's schema entry; empty when unknown or not a select. */
export function getSettingOptions(
    schema: SettingsSchema,
    key: string
): SelectOption[] {
    const definition = findSettingDefinition(schema, key);
    return definition?.kind === "select" ? definition.options : [];
}
