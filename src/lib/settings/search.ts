import type {
    SettingValue,
    SettingsSchema,
    SettingsSection,
} from "@/lib/backend/protocol";
import type {
    SettingKey,
    SettingsValues,
} from "@/lib/settings/settings.generated";
import { splitChord } from "@/lib/shortcuts";

import { visibleSettings, type VisibleSettingDefinition } from "./schema";

export interface SettingsSearchGroup {
    tabId: string;
    tabLabel: string;
    section: SettingsSection;
    settings: VisibleSettingDefinition[];
}

/**
 * Case-insensitive, tokenized search over visible settings. Every query
 * token must appear in the setting's label, description, or current value,
 * so "dark theme" still finds the theme select showing Dark.
 */
export function searchSettings(
    schema: SettingsSchema,
    values: SettingsValues,
    query: string
): SettingsSearchGroup[] {
    const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
    if (terms.length === 0) return [];

    const results: SettingsSearchGroup[] = [];
    for (const tab of schema.tabs) {
        for (const section of tab.sections) {
            const settings = visibleSettings(section).filter((definition) =>
                matchesSetting(definition, values, terms)
            );
            if (settings.length > 0) {
                results.push({
                    tabId: tab.id,
                    tabLabel: tab.label,
                    section,
                    settings,
                });
            }
        }
    }
    return results;
}

function matchesSetting(
    definition: VisibleSettingDefinition,
    values: SettingsValues,
    terms: string[]
): boolean {
    const value = values[definition.key as SettingKey];
    const haystack = [
        definition.label,
        definition.description ?? "",
        ...valueTerms(definition, value),
    ]
        .join(" ")
        .toLowerCase();

    return terms.every((term) => haystack.includes(term));
}

function valueTerms(
    definition: VisibleSettingDefinition,
    value: SettingValue | undefined
): string[] {
    switch (definition.kind) {
        case "boolean":
            if (typeof value !== "boolean") return [];
            return value
                ? ["on", "enabled", "true"]
                : ["off", "disabled", "false"];

        case "select": {
            const current = String(value ?? "");
            const option = definition.options.find(
                (candidate) => candidate.value === current
            );
            return option ? [current, option.label] : [current];
        }

        case "number":
            return typeof value === "number" ? [String(value)] : [];

        case "string":
        case "multilineString":
        case "font":
            return typeof value === "string" ? [value] : [];

        case "shortcut": {
            if (typeof value !== "string" || value.length === 0) return [];
            return [value, ...shortcutChords(value)];
        }
    }
}

const TOKEN_ALIASES: Record<string, readonly string[]> = {
    mod: ["mod", "ctrl", "cmd", "command"],
    ctrl: ["ctrl", "control"],
    cmd: ["cmd", "command", "meta"],
    meta: ["meta", "cmd", "command"],
    alt: ["alt", "option"],
    option: ["option", "alt"],
    enter: ["enter", "return"],
    escape: ["escape", "esc"],
};

/** Every alias spelling of a chord, so "ctrl+t" finds a stored "Mod+T". */
function shortcutChords(value: string): string[] {
    const tokens = splitChord(value);
    if (tokens.length === 0) return [];

    return tokens
        .map((token) => TOKEN_ALIASES[token.toLowerCase()] ?? [token])
        .reduce<string[]>(
            (chords, aliases) =>
                chords.flatMap((chord) =>
                    aliases.map((alias) => `${chord}+${alias}`)
                ),
            [""]
        )
        .map((chord) => chord.slice(1));
}
