import { remeasureDiffCharWidth } from "@/lib/diff/measure";
import { FONT_SETTINGS, fontFamilyStack } from "@/lib/fonts";
import { getSetting, settingsStore } from "@/stores/settings-store";

function applyFonts(): void {
    const root = document.documentElement;
    for (const setting of FONT_SETTINGS) {
        root.style.setProperty(
            setting.cssVariable,
            fontFamilyStack(getSetting(setting.key), setting.fallback)
        );
    }
    remeasureDiffCharWidth();
}

/**
 * Applies the stored font settings to the CSS variables and keeps them in
 * sync. Call once at startup AFTER settings are initialized but BEFORE the
 * first React render to avoid a font flash.
 */
export function startFontEngine(): void {
    settingsStore.subscribe(applyFonts);
    applyFonts();
}
