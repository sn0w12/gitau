import * as React from "react";
import type { ComponentType } from "react";

import {
    TooltipCreateHandle,
    TooltipPayloadHost,
    TooltipProvider,
} from "@/components/ui/tooltip";
import { useFixedHeightWindow } from "@/hooks/diff/use-fixed-height-window";
import { useViewportWidth } from "@/hooks/diff/use-viewport-width";
import type { DiffRow, SyntaxStyle } from "@/lib/backend/protocol";
import type {
    DiffDisplayItem,
    ExpandWindow,
    GapControls,
} from "@/lib/diff/context-gaps";
import { deriveSplitSegments, type SplitSegment } from "@/lib/diff/split-rows";
import { wrapContent, type WrappedLine } from "@/lib/diff/wrap";
import { cn } from "@/lib/utils";

import { DiffScroll } from "./diff-scroll";
import { HighlightedLine } from "./highlight-line";
import { HunkHeaderControls, type DiffTooltipHandle } from "./hunk-header";
import { DIFF_ROW_HEIGHT, KIND_ROW_CLASS, Lineno } from "./rows";

type SegmentLayout =
    | { type: "pending"; lines: 1 }
    | { type: "full"; row: DiffRow; lines: 1 }
    | {
          type: "pair";
          left?: DiffRow;
          right?: DiffRow;
          leftLines: WrappedLine[];
          rightLines: WrappedLine[];
          lines: number;
      };

/** Lineno gutter (w-10), the left cell's divider border, and rounding slack. */
const CONTENT_SLACK_PX = 40 + 1 + 2;

function layoutSegment(segment: SplitSegment, cpl: number): SegmentLayout {
    if (segment.type === "pending") return { type: "pending", lines: 1 };
    if (segment.type === "full") {
        return { type: "full", row: segment.row, lines: 1 };
    }
    const leftLines = segment.left
        ? wrapContent(segment.left.content, segment.left.spans, cpl)
        : [{ text: "" }];
    const rightLines = segment.right
        ? wrapContent(segment.right.content, segment.right.spans, cpl)
        : [{ text: "" }];
    return {
        type: "pair",
        left: segment.left,
        right: segment.right,
        leftLines,
        rightLines,
        lines: Math.max(leftLines.length, rightLines.length, 1),
    };
}

function FullRowView({
    row,
    controls,
    tooltipHandle,
    onExpand,
}: {
    row: DiffRow;
    controls: GapControls | null;
    tooltipHandle: DiffTooltipHandle;
    onExpand: (
        gapKey: number,
        direction: "up" | "down",
        window: ExpandWindow
    ) => void;
}) {
    return (
        <div
            style={{ height: DIFF_ROW_HEIGHT }}
            className={cn(
                "flex items-center text-xs whitespace-pre",
                KIND_ROW_CLASS[row.kind]
            )}
        >
            {controls ? (
                <HunkHeaderControls
                    controls={controls}
                    tooltipHandle={tooltipHandle}
                    onExpand={onExpand}
                />
            ) : (
                <span className="w-24 shrink-0" />
            )}
            <span className="truncate">
                {row.content === "" ? "\u00a0" : row.content}
            </span>
        </div>
    );
}

function PendingRowView() {
    return (
        <div
            style={{ height: DIFF_ROW_HEIGHT }}
            className="flex animate-pulse bg-muted/40"
        >
            <div className="w-1/2 border-r" />
            <div className="w-1/2" />
        </div>
    );
}

/**
 * One grid column of a wrapped pair: the row's visual lines stacked, with the
 * line number on the first line and an empty gutter on continuations. Missing
 * sides and filler lines below a shorter opposite side render as carbon.
 */
function WrappedCell({
    row,
    lines,
    totalLines,
    lineno,
    side,
    styles,
}: {
    row?: DiffRow;
    lines: WrappedLine[];
    totalLines: number;
    lineno?: number;
    side: "left" | "right";
    styles?: readonly SyntaxStyle[];
}) {
    const children = [];
    for (let k = 0; k < totalLines; k += 1) {
        const line = lines[k];
        children.push(
            <div
                key={k}
                style={{ height: DIFF_ROW_HEIGHT }}
                className={cn(
                    "flex items-center font-mono text-xs whitespace-pre",
                    row && line ? KIND_ROW_CLASS[row.kind] : "carbon"
                )}
            >
                {k === 0 ? (
                    <Lineno value={lineno} />
                ) : (
                    <span className="inline-block w-10 shrink-0" />
                )}
                <span className="code-hl min-w-0 flex-1 overflow-hidden">
                    {line ? (
                        <HighlightedLine
                            code={line.text}
                            row={row && { ...row, spans: line.spans }}
                            styles={styles}
                        />
                    ) : (
                        "\u00a0"
                    )}
                </span>
            </div>
        );
    }
    return (
        <div
            className={cn(
                "flex min-w-0 flex-col overflow-hidden",
                side === "left" && "border-r"
            )}
        >
            {children}
        </div>
    );
}

function PairView({
    layout,
    styles,
}: {
    layout: Extract<SegmentLayout, { type: "pair" }>;
    styles?: readonly SyntaxStyle[];
}) {
    return (
        <div
            style={{ height: layout.lines * DIFF_ROW_HEIGHT }}
            className="grid grid-cols-2 font-mono text-xs"
        >
            <WrappedCell
                row={layout.left}
                lines={layout.leftLines}
                totalLines={layout.lines}
                lineno={layout.left?.oldLineno}
                side="left"
                styles={styles}
            />
            <WrappedCell
                row={layout.right}
                lines={layout.rightLines}
                totalLines={layout.lines}
                lineno={layout.right?.newLineno}
                side="right"
                styles={styles}
            />
        </div>
    );
}

function SegmentView({
    layout,
    styles,
    controls,
    tooltipHandle,
    onExpand,
}: {
    layout: SegmentLayout;
    styles?: readonly SyntaxStyle[];
    controls: GapControls | null;
    tooltipHandle: DiffTooltipHandle;
    onExpand: (
        gapKey: number,
        direction: "up" | "down",
        window: ExpandWindow
    ) => void;
}) {
    if (layout.type === "pending") return <PendingRowView />;
    if (layout.type === "full") {
        return (
            <FullRowView
                row={layout.row}
                controls={controls}
                tooltipHandle={tooltipHandle}
                onExpand={onExpand}
            />
        );
    }
    return <PairView layout={layout} styles={styles} />;
}

const SplitSegmentRow = React.memo(function SplitSegmentRow({
    layout,
    styles,
    controls,
    tooltipHandle,
    onExpand,
    top,
}: {
    layout: SegmentLayout;
    styles?: readonly SyntaxStyle[];
    controls: GapControls | null;
    tooltipHandle: DiffTooltipHandle;
    onExpand: (
        gapKey: number,
        direction: "up" | "down",
        window: ExpandWindow
    ) => void;
    top: number;
}) {
    return (
        <div className="absolute right-0 left-0" style={{ top }}>
            <SegmentView
                layout={layout}
                styles={styles}
                controls={controls}
                tooltipHandle={tooltipHandle}
                onExpand={onExpand}
            />
        </div>
    );
});

export function SplitRows({
    items,
    charWidth,
    styles,
    onExpand,
}: {
    items: DiffDisplayItem[];
    charWidth: number;
    styles?: readonly SyntaxStyle[];
    onExpand: (
        gapKey: number,
        direction: "up" | "down",
        window: ExpandWindow
    ) => void;
}) {
    const segments = React.useMemo(
        () => deriveSplitSegments(items.map((item) => item.row)),
        [items]
    );
    const controlsByIndex = React.useMemo(() => {
        const map = new Map<number, GapControls>();
        items.forEach((item, index) => {
            if (item.controls) map.set(index, item.controls);
        });
        return map;
    }, [items]);
    const parentRef = React.useRef<HTMLDivElement>(null);
    const tooltipHandle = React.useMemo(
        () => TooltipCreateHandle<ComponentType>(),
        []
    );
    const viewportWidth = useViewportWidth(parentRef);

    const cpl = React.useMemo(() => {
        if (viewportWidth <= 0 || charWidth <= 0) {
            return Number.POSITIVE_INFINITY;
        }
        const contentWidthPx = Math.floor(viewportWidth / 2) - CONTENT_SLACK_PX;
        return contentWidthPx > 0
            ? Math.max(1, Math.floor(contentWidthPx / charWidth))
            : 1;
    }, [viewportWidth, charWidth]);

    const layouts = React.useMemo(
        () => segments.map((segment) => layoutSegment(segment, cpl)),
        [segments, cpl]
    );

    const lineStarts = React.useMemo(() => {
        const starts: number[] = [];
        let acc = 0;
        for (const layout of layouts) {
            starts.push(acc);
            acc += layout.lines;
        }
        return starts;
    }, [layouts]);

    const totalLines =
        layouts.length === 0
            ? 0
            : lineStarts[layouts.length - 1] +
              layouts[layouts.length - 1].lines;

    const { start, end, totalHeight } = useFixedHeightWindow({
        count: totalLines,
        rowHeight: DIFF_ROW_HEIGHT,
        containerRef: parentRef,
    });

    const nodes = [];
    if (layouts.length > 0) {
        // First segment whose visual span intersects the window; segments
        // are contiguous, so the largest lineStart below `start` contains it.
        let lo = 0;
        let hi = layouts.length - 1;
        while (lo < hi) {
            const mid = (lo + hi + 1) >> 1;
            if (lineStarts[mid] <= start) lo = mid;
            else hi = mid - 1;
        }
        for (let i = lo; i < layouts.length && lineStarts[i] < end; i += 1) {
            const layout = layouts[i];
            const segment = segments[i];
            const controls =
                segment.type === "full"
                    ? (controlsByIndex.get(segment.index) ?? null)
                    : null;
            nodes.push(
                <SplitSegmentRow
                    key={i}
                    layout={layout}
                    styles={styles}
                    controls={controls}
                    tooltipHandle={tooltipHandle}
                    onExpand={onExpand}
                    top={lineStarts[i] * DIFF_ROW_HEIGHT}
                />
            );
        }
    }

    return (
        <TooltipProvider>
            <div className="relative min-h-0 min-w-0 flex-1">
                <DiffScroll
                    viewportRef={parentRef}
                    totalHeight={totalHeight}
                    paddingTop={0}
                >
                    {nodes}
                </DiffScroll>
            </div>
            <TooltipPayloadHost handle={tooltipHandle} />
        </TooltipProvider>
    );
}
