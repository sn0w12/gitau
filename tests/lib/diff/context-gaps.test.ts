import { describe, expect, it } from "vitest";

import type { DiffRow } from "@/lib/backend/protocol";
import type { SectionGapExpansions } from "@/lib/backend/streams/diff-session";
import {
    buildDisplayItems,
    computeDiffGaps,
    CONTEXT_EXPAND_LINES,
    parseHunkHeader,
} from "@/lib/diff/context-gaps";

function row(
    kind: DiffRow["kind"],
    content: string,
    numbers?: { old?: number; new?: number }
): DiffRow {
    return {
        kind,
        content,
        ...(numbers?.old !== undefined ? { oldLineno: numbers.old } : {}),
        ...(numbers?.new !== undefined ? { newLineno: numbers.new } : {}),
    };
}

describe("parseHunkHeader", () => {
    it("reads explicit counts", () => {
        expect(parseHunkHeader("@@ -1,3 +4,5 @@ fn main")).toEqual({
            oldStart: 1,
            oldCount: 3,
            newStart: 4,
            newCount: 5,
        });
    });

    it("defaults an omitted count to one", () => {
        expect(parseHunkHeader("@@ -7 +9 @@")).toEqual({
            oldStart: 7,
            oldCount: 1,
            newStart: 9,
            newCount: 1,
        });
    });

    it("returns null for non-hunk rows", () => {
        expect(parseHunkHeader("+added")).toBeNull();
    });
});

describe("computeDiffGaps", () => {
    it("finds the hidden run between two hunks", () => {
        const rows = [
            row("hunkHeader", "@@ -1,3 +1,3 @@"),
            row("context", "a", { old: 1, new: 1 }),
            row("context", "b", { old: 2, new: 2 }),
            row("context", "c", { old: 3, new: 3 }),
            row("hunkHeader", "@@ -10,3 +10,3 @@"),
            row("context", "x", { old: 10, new: 10 }),
        ];
        expect(computeDiffGaps(rows, undefined)).toEqual([
            {
                key: 4,
                oldStart: 4,
                oldEnd: 9,
                newStart: 4,
                newEnd: 9,
                count: 6,
            },
        ]);
    });

    it("adds a trailing gap when the layout reports more old lines", () => {
        const rows = [
            row("hunkHeader", "@@ -1,2 +1,2 @@"),
            row("context", "a", { old: 1, new: 1 }),
            row("context", "b", { old: 2, new: 2 }),
        ];
        expect(computeDiffGaps(rows, 5)).toEqual([
            {
                key: 3,
                oldStart: 3,
                oldEnd: 5,
                newStart: 3,
                newEnd: 5,
                count: 3,
            },
        ]);
    });

    it("finds no gaps for a pure addition file", () => {
        const rows = [
            row("hunkHeader", "@@ -0,0 +1,2 @@"),
            row("addition", "a", { new: 1 }),
            row("addition", "b", { new: 2 }),
        ];
        expect(computeDiffGaps(rows, 0)).toEqual([]);
    });
});

describe("buildDisplayItems", () => {
    const rows = [
        row("hunkHeader", "@@ -1,3 +1,3 @@"),
        row("context", "a", { old: 1, new: 1 }),
        row("context", "b", { old: 2, new: 2 }),
        row("context", "c", { old: 3, new: 3 }),
        row("hunkHeader", "@@ -10,3 +10,3 @@"),
    ];

    it("attaches the gap's controls to the bordering hunk headers", () => {
        const gaps = computeDiffGaps(rows, undefined);
        const items = buildDisplayItems(rows, gaps, undefined, true);
        expect(items).toHaveLength(5);
        expect(items[0].row?.kind).toBe("hunkHeader");
        expect(items[0].controls).toEqual({
            up: null,
            down: {
                gapKey: 4,
                direction: "down",
                pending: false,
                window: { oldStart: 4, newStart: 4, count: 6 },
            },
        });
        expect(items[1].controls).toBeNull();
        expect(items[4].controls).toEqual({
            up: {
                gapKey: 4,
                direction: "up",
                pending: false,
                window: { oldStart: 4, newStart: 4, count: 6 },
            },
            down: null,
        });
    });

    it("gives both bordering hunks a control for a large gap", () => {
        const rows40 = [
            row("hunkHeader", "@@ -1,1 +1,1 @@"),
            row("context", "a", { old: 1, new: 1 }),
            row("hunkHeader", "@@ -52,1 +52,1 @@"),
        ];
        const gaps = computeDiffGaps(rows40, undefined);
        const items = buildDisplayItems(rows40, gaps, undefined, true);
        expect(items[0].controls?.down?.window).toEqual({
            oldStart: 2,
            newStart: 2,
            count: CONTEXT_EXPAND_LINES,
        });
        expect(items[2].controls?.up?.window).toEqual({
            oldStart: 52 - CONTEXT_EXPAND_LINES,
            newStart: 52 - CONTEXT_EXPAND_LINES,
            count: CONTEXT_EXPAND_LINES,
        });
    });

    it("inserts the revealed rows before the following hunk header", () => {
        const rows40 = [
            row("hunkHeader", "@@ -1,1 +1,1 @@"),
            row("context", "a", { old: 1, new: 1 }),
            row("hunkHeader", "@@ -52,1 +52,1 @@"),
        ];
        const gaps = computeDiffGaps(rows40, undefined);
        const expansions: SectionGapExpansions = new Map([
            [
                2,
                {
                    rows: [
                        row("context", "p", { old: 10, new: 10 }),
                        row("context", "q", { old: 11, new: 11 }),
                    ],
                    oldStart: 10,
                    oldEnd: 11,
                    pendingUp: false,
                    pendingDown: false,
                },
            ],
        ]);
        const items = buildDisplayItems(rows40, gaps, expansions, true);
        expect(items.map((item) => item.row?.content)).toEqual([
            "@@ -1,1 +1,1 @@",
            "a",
            "p",
            "q",
            "@@ -52,1 +52,1 @@",
        ]);
        expect(items[0].controls?.down?.window).toEqual({
            oldStart: 12,
            newStart: 12,
            count: CONTEXT_EXPAND_LINES,
        });
        expect(items[4].controls?.up?.window).toEqual({
            oldStart: 2,
            newStart: 2,
            count: 8,
        });
    });

    it("omits controls while the section is still streaming", () => {
        const gaps = computeDiffGaps(rows, undefined);
        const items = buildDisplayItems(rows, gaps, undefined, false);
        expect(items.every((item) => item.controls === null)).toBe(true);
    });
});
