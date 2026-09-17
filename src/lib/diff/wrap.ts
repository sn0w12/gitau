/**
 * One visual line of a wrapped diff row: a slice of the row's content with
 * the backend span triples re-based to the slice, so each visual line can be
 * syntax-highlighted independently.
 */
export interface WrappedLine {
    text: string;
    spans?: number[];
}

/**
 * Rows are sliced here instead of letting CSS `pre-wrap` wrap them: browsers
 * apply UAX #14 line breaking (breaks after hyphens, slashes, and more) that
 * cannot be predicted from character counts, and the virtualizer needs the
 * exact line count for each row's height. Monospace glyphs share one advance,
 * so a column budget plus tab stops reproduces the layout exactly and the
 * rendered slices can stay `white-space: pre`. Matches `overflow-wrap:
 * anywhere` semantics: break at the last space that fits, or anywhere when the
 * line has none.
 */
export const TAB_SIZE = 8;

function sliceSpans(
    spans: readonly number[] | undefined,
    start: number,
    end: number
): number[] | undefined {
    if (!spans) return undefined;
    const out: number[] = [];
    for (let i = 0; i < spans.length; i += 3) {
        const spanStart = spans[i];
        const len = spans[i + 1];
        const styleId = spans[i + 2];
        const a = Math.max(spanStart, start);
        const b = Math.min(spanStart + len, end);
        if (a < b) out.push(a - start, b - a, styleId);
    }
    return out.length > 0 ? out : undefined;
}

export function wrapContent(
    content: string,
    spans: readonly number[] | undefined,
    cpl: number
): WrappedLine[] {
    const cleaned = content.replace(/\r/g, "");
    if (!Number.isFinite(cpl) || cleaned.length === 0) {
        return [{ text: cleaned, spans: spans ? [...spans] : undefined }];
    }
    const lines: WrappedLine[] = [];
    let start = 0;
    while (start < cleaned.length) {
        let col = 0;
        let fit = start;
        let lastSpace = -1;
        for (let i = start; i < cleaned.length; i += 1) {
            const ch = cleaned[i];
            const w = ch === "\t" ? TAB_SIZE - (col % TAB_SIZE) : 1;
            if (col + w > cpl) break;
            col += w;
            fit = i + 1;
            if (ch === " ") lastSpace = i;
        }
        let end: number;
        if (fit === cleaned.length) {
            end = cleaned.length;
        } else if (fit === start) {
            // A tab wider than the budget still consumes one char.
            end = start + 1;
        } else if (lastSpace >= start) {
            end = lastSpace + 1;
        } else {
            end = fit;
        }
        lines.push({
            text: cleaned.slice(start, end),
            spans: sliceSpans(spans, start, end),
        });
        start = end;
    }
    return lines;
}
