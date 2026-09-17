import { describe, expect, it } from "vitest";

import { segmentsFromSpans } from "@/components/diff/highlight-line";

describe("segmentsFromSpans", () => {
    it("splits styled and plain segments", () => {
        // "let x" -> "let" styled, " x" plain
        const spans = [0, 3, 1];
        const code = "let x";
        expect(segmentsFromSpans(code, spans)).toEqual([
            { text: "let", styleId: 1 },
            { text: " x", styleId: null },
        ]);
    });

    it("keeps gaps between spans as plain text", () => {
        const spans = [0, 2, 3, 5, 2, 4];
        const code = "abcdefg";
        expect(segmentsFromSpans(code, spans)).toEqual([
            { text: "ab", styleId: 3 },
            { text: "cde", styleId: null },
            { text: "fg", styleId: 4 },
        ]);
    });

    it("returns null for missing or malformed data", () => {
        expect(segmentsFromSpans("abc", undefined)).toBeNull();
        expect(segmentsFromSpans("abc", [])).toBeNull();
        expect(segmentsFromSpans("abc", [0, 2])).toBeNull();
        expect(segmentsFromSpans("abc", [0, 4, 1])).toBeNull();
        expect(segmentsFromSpans("abc", [0, 1, 0])).toBeNull();
        expect(segmentsFromSpans("abc", [0, 1, 1.5])).toBeNull();
    });

    it("accepts full coverage without trailing plain segment", () => {
        expect(segmentsFromSpans("ab", [0, 2, 7])).toEqual([
            { text: "ab", styleId: 7 },
        ]);
    });
});
