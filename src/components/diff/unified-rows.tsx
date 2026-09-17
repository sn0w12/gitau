import * as React from "react";

import { useFixedHeightWindow } from "@/hooks/diff/use-fixed-height-window";
import type { DiffRow, SyntaxStyle } from "@/lib/backend/protocol";
import { cn } from "@/lib/utils";

import { DiffScroll } from "./diff-scroll";
import { HighlightedLine } from "./highlight-line";
import {
    DIFF_ROW_HEIGHT,
    KIND_ROW_BORDER_CLASS,
    KIND_ROW_CLASS,
    Lineno,
} from "./rows";

/** Two lineno gutters plus the +/- marker column. */
const GUTTER_PX = 2 * 40 + 16;

function LineContent({
    row,
    styles,
}: {
    row?: DiffRow;
    styles?: readonly SyntaxStyle[];
}) {
    if (!row) return <span>&nbsp;</span>;
    return (
        <span className="diff-hl-code pl-1 whitespace-pre">
            <HighlightedLine code={row.content} row={row} styles={styles} />
        </span>
    );
}

type RowsInput = {
    rows: (DiffRow | undefined)[];
    contentMinWidthPx: number;
    styles?: readonly SyntaxStyle[];
};

export function UnifiedRows({ rows, contentMinWidthPx, styles }: RowsInput) {
    const parentRef = React.useRef<HTMLDivElement>(null);
    const { start, end, totalHeight, offsetTop } = useFixedHeightWindow({
        count: rows.length,
        rowHeight: DIFF_ROW_HEIGHT,
        containerRef: parentRef,
    });

    const items = [];
    for (let i = start; i < end; i += 1) {
        const row = rows[i];
        items.push(
            <div
                key={i}
                style={{ height: DIFF_ROW_HEIGHT }}
                className={cn(
                    "flex items-center whitespace-pre",
                    row
                        ? KIND_ROW_CLASS[row.kind]
                        : "animate-pulse bg-muted/40",
                    row ? `border-l-3 ${KIND_ROW_BORDER_CLASS[row.kind]}` : ""
                )}
            >
                <Lineno value={row?.oldLineno} />
                <Lineno value={row?.newLineno} />
                <span className="w-4 shrink-0 text-center opacity-60 select-none">
                    {row?.kind === "addition"
                        ? "+"
                        : row?.kind === "deletion"
                          ? "\u2212"
                          : ""}
                </span>
                <LineContent row={row} styles={styles} />
            </div>
        );
    }

    return (
        <div className="relative min-h-0 min-w-0 flex-1">
            <DiffScroll
                viewportRef={parentRef}
                totalHeight={totalHeight}
                paddingTop={offsetTop}
                contentMinWidthPx={contentMinWidthPx + GUTTER_PX}
            >
                {items}
            </DiffScroll>
        </div>
    );
}
