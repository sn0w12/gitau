import type { SettingKey } from "@/lib/settings/settings.generated";

export interface FontSettingSpec {
    key: Extract<
        SettingKey,
        "fontSans" | "fontMono" | "fontHeading" | "fontDisplay"
    >;
    cssVariable: string;
    fallback: "sans-serif" | "serif" | "monospace";
}

export const FONT_SETTINGS: readonly FontSettingSpec[] = [
    { key: "fontSans", cssVariable: "--app-font-sans", fallback: "sans-serif" },
    { key: "fontMono", cssVariable: "--app-font-mono", fallback: "monospace" },
    {
        key: "fontHeading",
        cssVariable: "--app-font-heading",
        fallback: "serif",
    },
    {
        key: "fontDisplay",
        cssVariable: "--app-font-display",
        fallback: "serif",
    },
];

/** Bundled through @fontsource in styles.css; listed before system fonts. */
export const BUNDLED_FONTS = [
    "Inter Variable",
    "Geist Mono Variable",
    "Kalnia Variable",
    "Instrument Serif",
] as const;

/**
 * A CSS `font-family` value for one family name. Quoted so names with
 * spaces or punctuation resolve as a single family, with the generic
 * fallback kept behind it.
 */
export function fontFamilyStack(family: string, fallback: string): string {
    const trimmed = family.trim();
    if (trimmed.length === 0) return fallback;
    const escaped = trimmed.replaceAll("\\", "\\\\").replaceAll('"', '\\"');
    return `"${escaped}", ${fallback}`;
}

/** Bundled fonts first, then installed ones, deduplicated, keeping a
 * selected family that is no longer installed selectable. */
export function fontChoices(
    systemFonts: readonly string[],
    selected: string
): string[] {
    const seen = new Set<string>();
    const families: string[] = [];
    for (const family of [...BUNDLED_FONTS, ...systemFonts, selected]) {
        const trimmed = family.trim();
        if (trimmed.length === 0 || seen.has(trimmed)) continue;
        seen.add(trimmed);
        families.push(trimmed);
    }
    return families;
}
