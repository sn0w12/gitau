import type { DiffRow } from "@/lib/backend/protocol";
import type { SectionGapExpansions } from "@/lib/backend/streams/diff-session";

export const CONTEXT_EXPAND_LINES = 20;

export interface HunkRange {
    oldStart: number;
    oldCount: number;
    newStart: number;
    newCount: number;
}

const HUNK_HEADER = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/;

/** Parses `@@ -a,b +c,d @@ [heading]`; omitted counts mean one line. */
export function parseHunkHeader(content: string): HunkRange | null {
    const match = HUNK_HEADER.exec(content);
    if (!match) return null;
    return {
        oldStart: Number(match[1]),
        oldCount: match[2] === undefined ? 1 : Number(match[2]),
        newStart: Number(match[3]),
        newCount: match[4] === undefined ? 1 : Number(match[4]),
    };
}

/** A run of unchanged lines hidden between two hunks (or a file boundary). */
export interface DiffGap {
    /** Row index the gap precedes: the following hunk header, or row count. */
    key: number;
    oldStart: number;
    oldEnd: number;
    newStart: number;
    newEnd: number;
    count: number;
}

/**
 * Derives the hidden context gaps from a section's display rows. Old and new
 * ranges stay the same length across a gap because unchanged lines advance
 * both sides together.
 */
export function computeDiffGaps(
    rows: readonly (DiffRow | undefined)[],
    totalOldLines: number | undefined
): DiffGap[] {
    const hunks: { index: number; range: HunkRange }[] = [];
    rows.forEach((row, index) => {
        if (row?.kind !== "hunkHeader") return;
        const range = parseHunkHeader(row.content);
        if (range) hunks.push({ index, range });
    });
    if (hunks.length === 0) return [];

    const gaps: DiffGap[] = [];
    let lastOldEnd = 0;
    let lastNewEnd = 0;
    for (const { index, range } of hunks) {
        const { oldStart, oldCount, newStart, newCount } = range;
        const oldGapStart = lastOldEnd + 1;
        const oldGapEnd = oldStart - 1;
        const newGapStart = lastNewEnd + 1;
        const newGapEnd = newStart - 1;
        if (oldGapEnd >= oldGapStart && newGapEnd >= newGapStart) {
            gaps.push({
                key: index,
                oldStart: oldGapStart,
                oldEnd: oldGapEnd,
                newStart: newGapStart,
                newEnd: newGapEnd,
                count: oldGapEnd - oldGapStart + 1,
            });
        }
        lastOldEnd = oldCount === 0 ? oldStart : oldStart + oldCount - 1;
        lastNewEnd = newCount === 0 ? newStart : newStart + newCount - 1;
    }

    if (totalOldLines !== undefined) {
        const count = totalOldLines - lastOldEnd;
        if (count > 0) {
            gaps.push({
                key: rows.length,
                oldStart: lastOldEnd + 1,
                oldEnd: totalOldLines,
                newStart: lastNewEnd + 1,
                newEnd: lastNewEnd + count,
                count,
            });
        }
    }
    return gaps;
}

/** A window of lines to request from the backend. */
export interface ExpandWindow {
    oldStart: number;
    newStart: number;
    count: number;
}

export interface ControlSpec {
    gapKey: number;
    /** "up" reveals the hidden lines above a hunk, "down" those below it. */
    direction: "up" | "down";
    window: ExpandWindow;
    pending: boolean;
}

export interface GapControls {
    up: ControlSpec | null;
    down: ControlSpec | null;
}

export interface DiffDisplayItem {
    row: DiffRow | undefined;
    /** Set only on hunk headers that border a gap with hidden lines. */
    controls: GapControls | null;
}

/**
 * Interleaves revealed context rows into a section's base rows and attaches
 * expand controls to each hunk header: "up" grows the gap above the hunk from
 * its lower edge, "down" grows the gap below it from its upper edge. Because
 * both directions share one contiguous revealed block per gap, the two
 * controls never offer the same lines.
 */
export function buildDisplayItems(
    rows: readonly (DiffRow | undefined)[],
    gaps: readonly DiffGap[],
    expansions: SectionGapExpansions | undefined,
    canExpand: boolean
): DiffDisplayItem[] {
    const gapByKey = new Map<number, DiffGap>();
    for (const gap of gaps) gapByKey.set(gap.key, gap);

    const hunkIndices: number[] = [];
    rows.forEach((row, index) => {
        if (row?.kind === "hunkHeader" && parseHunkHeader(row.content)) {
            hunkIndices.push(index);
        }
    });
    const upGapFor = new Map<number, DiffGap>();
    const downGapFor = new Map<number, DiffGap>();
    hunkIndices.forEach((index, position) => {
        const above = gapByKey.get(index);
        if (above) upGapFor.set(index, above);
        const belowKey =
            position + 1 < hunkIndices.length
                ? hunkIndices[position + 1]
                : rows.length;
        const below = gapByKey.get(belowKey);
        if (below) downGapFor.set(index, below);
    });

    const items: DiffDisplayItem[] = [];
    const pushRevealed = (gap: DiffGap) => {
        for (const row of expansions?.get(gap.key)?.rows ?? []) {
            items.push({ row, controls: null });
        }
    };

    for (let i = 0; i < rows.length; i += 1) {
        const gap = gapByKey.get(i);
        if (gap) pushRevealed(gap);
        items.push({ row: rows[i], controls: controlsFor(i, canExpand) });
    }
    const tail = gapByKey.get(rows.length);
    if (tail) pushRevealed(tail);
    return items;

    function controlsFor(index: number, enabled: boolean): GapControls | null {
        if (!enabled) return null;
        const above = upGapFor.get(index);
        const below = downGapFor.get(index);
        const up = above ? upControl(above, expansions) : null;
        const down = below ? downControl(below, expansions) : null;
        return up || down ? { up, down } : null;
    }
}

function upControl(
    gap: DiffGap,
    expansions: SectionGapExpansions | undefined
): ControlSpec | null {
    const expansion = expansions?.get(gap.key);
    const blockStart = expansion?.oldStart ?? gap.oldEnd + 1;
    const count = Math.min(CONTEXT_EXPAND_LINES, blockStart - gap.oldStart);
    if (count <= 0) return null;
    const oldStart = blockStart - count;
    return {
        gapKey: gap.key,
        direction: "up",
        pending: expansion?.pendingUp ?? false,
        window: {
            oldStart,
            newStart: gap.newStart + (oldStart - gap.oldStart),
            count,
        },
    };
}

function downControl(
    gap: DiffGap,
    expansions: SectionGapExpansions | undefined
): ControlSpec | null {
    const expansion = expansions?.get(gap.key);
    const blockEnd = expansion?.oldEnd ?? gap.oldStart - 1;
    const count = Math.min(CONTEXT_EXPAND_LINES, gap.oldEnd - blockEnd);
    if (count <= 0) return null;
    const oldStart = blockEnd + 1;
    return {
        gapKey: gap.key,
        direction: "down",
        pending: expansion?.pendingDown ?? false,
        window: {
            oldStart,
            newStart: gap.newStart + (oldStart - gap.oldStart),
            count,
        },
    };
}
