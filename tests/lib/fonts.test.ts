import { describe, expect, it } from "vitest";

import { fontChoices, fontFamilyStack } from "@/lib/fonts";

describe("fontFamilyStack", () => {
    it("quotes the family and keeps the generic fallback", () => {
        expect(fontFamilyStack("Cascadia Code", "monospace")).toBe(
            '"Cascadia Code", monospace'
        );
    });

    it("escapes quotes and backslashes in the family name", () => {
        expect(fontFamilyStack('A"B\\C', "sans-serif")).toBe(
            '"A\\"B\\\\C", sans-serif'
        );
    });

    it("returns only the fallback for an empty family", () => {
        expect(fontFamilyStack("   ", "serif")).toBe("serif");
    });
});

describe("fontChoices", () => {
    it("lists bundled fonts before installed ones", () => {
        expect(
            fontChoices(["Segoe UI", "Inter Variable"], "Inter Variable")
        ).toEqual([
            "Inter Variable",
            "Geist Mono Variable",
            "Kalnia Variable",
            "Instrument Serif",
            "Segoe UI",
        ]);
    });

    it("keeps the selected family when it is no longer installed", () => {
        expect(fontChoices(["Segoe UI"], "Retired Font")).toContain(
            "Retired Font"
        );
    });

    it("drops blank and duplicate entries", () => {
        expect(
            fontChoices(["", "Segoe UI", " Segoe UI "], "Inter Variable")
        ).toEqual([
            "Inter Variable",
            "Geist Mono Variable",
            "Kalnia Variable",
            "Instrument Serif",
            "Segoe UI",
        ]);
    });
});
