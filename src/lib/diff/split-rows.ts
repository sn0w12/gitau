import type { DiffRow } from "@/lib/backend/protocol";
import type { SectionRows } from "@/lib/backend/streams/diff-session";

/**
 * One output row of the side-by-side view. Full-width rows (file/hunk
 * headers, binary notices) span both columns; everything else pairs a
 * deletion with an addition where possible. `undefined` sides mean "no line
 * on this side" or "not streamed yet".
 */
export type SplitSegment =
    | { type: "full"; row: DiffRow; index: number }
    | {
          type: "pair";
          left?: DiffRow;
          right?: DiffRow;
          leftIndex?: number;
          rightIndex?: number;
      }
    | { type: "pending" };

/** Rows that span both columns instead of pairing. */
export function isFullWidth(row: DiffRow): boolean {
    return (
        row.kind === "fileHeader" ||
        row.kind === "hunkHeader" ||
        row.kind === "binaryNotice"
    );
}

function isDeletion(row: DiffRow | undefined): boolean {
    return row?.kind === "deletion";
}

function isAddition(row: DiffRow | undefined): boolean {
    return row?.kind === "addition";
}

/**
 * Derives side-by-side segments from a dense section-row slice. Runs of
 * deletions pair index-wise against the following run of additions; leftovers
 * get an empty opposite cell. Holes (`undefined`, rows that have not streamed
 * yet) become pending segments so partial data renders as skeletons without
 * mispairing across chunk boundaries.
 */
export function deriveSplitSegments(
    rows: readonly (DiffRow | undefined)[]
): SplitSegment[] {
    const segments: SplitSegment[] = [];
    let i = 0;
    while (i < rows.length) {
        const row = rows[i];
        if (!row) {
            segments.push({ type: "pending" });
            i += 1;
            continue;
        }
        if (isFullWidth(row)) {
            segments.push({ type: "full", row, index: i });
            i += 1;
            continue;
        }
        if (isDeletion(row)) {
            const deletions: DiffRow[] = [];
            while (isDeletion(rows[i])) deletions.push(rows[i++]!);
            const deletionStart = i - deletions.length;
            const additions: DiffRow[] = [];
            while (isAddition(rows[i])) additions.push(rows[i++]!);
            const additionStart = i - additions.length;
            const pairs = Math.max(deletions.length, additions.length);
            for (let k = 0; k < pairs; k += 1) {
                segments.push({
                    type: "pair",
                    left: deletions[k],
                    right: additions[k],
                    leftIndex: deletionStart + k,
                    rightIndex: additionStart + k,
                });
            }
            continue;
        }
        if (isAddition(row)) {
            segments.push({ type: "pair", right: row, rightIndex: i });
            i += 1;
            continue;
        }
        segments.push({
            type: "pair",
            left: row,
            right: row,
            leftIndex: i,
            rightIndex: i,
        });
        i += 1;
    }
    return segments;
}

/**
 * Expands the sparse bucketed store representation into a dense slice sized
 * `rowCount`; holes stay `undefined` so callers can render placeholders.
 */
export function materializeSectionRows(
    buckets: SectionRows | undefined,
    rowCount: number
): (DiffRow | undefined)[] {
    const out: (DiffRow | undefined)[] = Array.from(
        { length: rowCount },
        () => undefined
    );
    if (!buckets) return out;
    for (const [start, bucket] of buckets) {
        for (let k = 0; k < bucket.length; k += 1) {
            const index = start + k;
            if (index >= rowCount) break;
            if (bucket[k] !== undefined && out[index] === undefined) {
                out[index] = bucket[k];
            }
        }
    }
    return out;
}
