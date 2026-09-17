import { describe, expect, it } from "vitest";

import { wrapContent } from "@/lib/diff/wrap";

describe("wrapContent", () => {
    it("keeps short lines whole", () => {
        expect(wrapContent("short", undefined, 80)).toEqual([
            { text: "short" },
        ]);
    });

    it("splits unbroken lines at the column budget", () => {
        const lines = wrapContent("a".repeat(120), undefined, 50);
        expect(lines.map((l) => l.text)).toEqual([
            "a".repeat(50),
            "a".repeat(50),
            "a".repeat(20),
        ]);
    });

    it("breaks at the last space that fits, not the budget", () => {
        // 9 columns: "aa bbb ccc" fills 9, the next char would overflow, so
        // the break falls back to the space after "aa bbb".
        const lines = wrapContent("aa bbb ccc ddd", undefined, 9);
        expect(lines.map((l) => l.text)).toEqual(["aa bbb ", "ccc ddd"]);
    });

    it("advances tabs to the next tab stop", () => {
        // A tab from column 0 spans 8 columns, so "a" starts the next line.
        expect(wrapContent("\ta", undefined, 8).map((l) => l.text)).toEqual([
            "\t",
            "a",
        ]);
        // 4 chars then a tab reaches column 8; the rest fits in a 12-wide pane.
        expect(wrapContent("abcd\tefgh", undefined, 12)).toEqual([
            { text: "abcd\tefgh" },
        ]);
    });

    it("re-bases span triples onto each slice", () => {
        const content = "abcdefghij";
        const lines = wrapContent(content, [0, 4, 1, 6, 3, 2], 5);
        expect(lines).toEqual([
            { text: "abcde", spans: [0, 4, 1] },
            { text: "fghij", spans: [1, 3, 2] },
        ]);
    });

    it("does not wrap with an infinite budget", () => {
        const content = "abc\td";
        expect(
            wrapContent(content, undefined, Number.POSITIVE_INFINITY)
        ).toEqual([{ text: content }]);
    });

    it("yields one empty line for empty content", () => {
        expect(wrapContent("", undefined, 10)).toEqual([{ text: "" }]);
    });
});
