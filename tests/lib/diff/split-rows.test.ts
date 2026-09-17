import { describe, expect, it } from "vitest";

import type { DiffRow } from "@/lib/backend/protocol";
import {
    deriveSplitSegments,
    materializeSectionRows,
} from "@/lib/diff/split-rows";

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

describe("deriveSplitSegments", () => {
    it("pairs deletion runs against addition runs index-wise", () => {
        const rows = [
            row("hunkHeader", "@@ -1,3 +1,3 @@"),
            row("context", "a", { old: 1, new: 1 }),
            row("deletion", "b", { old: 2 }),
            row("deletion", "c", { old: 3 }),
            row("addition", "B2", { new: 2 }),
            row("context", "d", { old: 4, new: 3 }),
        ];
        const segments = deriveSplitSegments(rows);
        expect(segments).toEqual([
            { type: "full", row: rows[0], index: 0 },
            {
                type: "pair",
                left: rows[1],
                right: rows[1],
                leftIndex: 1,
                rightIndex: 1,
            },
            {
                type: "pair",
                left: rows[2],
                right: rows[4],
                leftIndex: 2,
                rightIndex: 4,
            },
            {
                type: "pair",
                left: rows[3],
                right: undefined,
                leftIndex: 3,
                rightIndex: 5,
            },
            {
                type: "pair",
                left: rows[5],
                right: rows[5],
                leftIndex: 5,
                rightIndex: 5,
            },
        ]);
    });

    it("gives pure-addition runs empty left cells", () => {
        const segments = deriveSplitSegments([
            row("addition", "x", { new: 1 }),
            row("addition", "y", { new: 2 }),
        ]);
        expect(segments).toHaveLength(2);
        expect(segments[0]).toEqual({
            type: "pair",
            right: expect.objectContaining({ content: "x" }),
            rightIndex: 0,
        });
        expect(segments[1]).toMatchObject({
            right: { content: "y" },
            rightIndex: 1,
        });
    });

    it("emits full-width rows unchanged", () => {
        const header = row("fileHeader", "diff --git a/f b/f");
        const notice = row("binaryNotice", "Binary file differs");
        const segments = deriveSplitSegments([header, notice]);
        expect(segments).toEqual([
            { type: "full", row: header, index: 0 },
            { type: "full", row: notice, index: 1 },
        ]);
    });

    it("does not pair across holes; holes become pending segments", () => {
        const rows = [
            row("deletion", "b", { old: 2 }),
            undefined,
            row("addition", "B", { new: 2 }),
        ];
        const segments = deriveSplitSegments(rows);
        expect(segments).toEqual([
            {
                type: "pair",
                left: rows[0],
                right: undefined,
                leftIndex: 0,
                rightIndex: 1,
            },
            { type: "pending" },
            { type: "pair", right: rows[2], rightIndex: 2 },
        ]);
    });

    it("keeps newline markers inside their runs", () => {
        const marker = row("addition", "\\ No newline at end of file", {
            new: 3,
        });
        const segments = deriveSplitSegments([
            row("addition", "tail", { new: 3 }),
            marker,
        ]);
        expect(segments).toEqual([
            { type: "pair", right: expect.anything(), rightIndex: 0 },
            { type: "pair", right: marker, rightIndex: 1 },
        ]);
    });
});

describe("materializeSectionRows", () => {
    it("expands sparse buckets into a dense slice with holes", () => {
        const buckets = new Map<number, (DiffRow | undefined)[]>([
            [256, [row("context", "later")]],
            [0, [row("context", "first"), undefined]],
        ]);
        const rows = materializeSectionRows(buckets, 258);
        expect(rows).toHaveLength(258);
        expect(rows[0]?.content).toBe("first");
        expect(rows[1]).toBeUndefined();
        expect(rows[256]?.content).toBe("later");
    });

    it("returns an all-hole slice without buckets", () => {
        const rows = materializeSectionRows(undefined, 3);
        expect(rows).toEqual([undefined, undefined, undefined]);
    });
});
