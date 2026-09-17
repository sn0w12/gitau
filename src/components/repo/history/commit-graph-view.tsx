import { useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties } from "react";

import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { useTabId } from "@/contexts/tab-context";
import { useGraphSession } from "@/hooks/repositories/use-graph-session";
import type { GraphRow } from "@/lib/backend/protocol";
import { formatRelativeDate } from "@/lib/utils";

import { Badge } from "../../ui/badge";

export const GRAPH_ROW_HEIGHT_PX = 32;
const GRAPH_OVERSCAN_ROWS = 10;
/** Catch-up read size when a completed session still has missing rows. */
const GRAPH_CATCHUP_ROWS = 2048;
const LANE_COLORS = 8;
const LANE_GAP = 20;
const LANE_PAD = 20;
/** Column left of the graph holding ref pills and their dashed connectors. */
const REF_ZONE_WIDTH = 300;
const EDGE_STROKE = 2;
const NODE_RADIUS = 7;
const MERGE_RADIUS = 8;
/** Fixed-width meta columns right of the summary, outside the row tint. */
const META_WIDTH = 450;
const META_TOTAL = META_WIDTH + 16;

export function laneVar(lane: number): string {
    return `var(--graph-lane-${((lane % LANE_COLORS) + LANE_COLORS) % LANE_COLORS})`;
}

function laneX(lane: number): number {
    return REF_ZONE_WIDTH + LANE_PAD + lane * LANE_GAP;
}

function laneAlpha(lane: number, percent: number): string {
    return `color-mix(in srgb, ${laneVar(lane)} ${percent}%, transparent)`;
}

/** Reassembles the contiguous streamed prefix from chunk storage. */
export function flattenRows(
    chunks: Map<number, GraphRow[]>,
    knownTotalRows: number
): GraphRow[] {
    const rows: GraphRow[] = [];
    let cursor = 0;
    const starts = [...chunks.keys()].sort((a, b) => a - b);
    for (const start of starts) {
        if (start > cursor) break;
        const chunk = chunks.get(start) as GraphRow[];
        if (start + chunk.length <= cursor) continue;
        rows.push(...chunk.slice(cursor - start));
        cursor = start + chunk.length;
    }
    return rows.length <= knownTotalRows ? rows : rows.slice(0, knownTotalRows);
}

function shortId(id: string): string {
    return id.slice(0, 7);
}
/**
 * Fixed-row-height commit graph under the history chart. Rows are absolutely
 * positioned at `index * GRAPH_ROW_HEIGHT_PX`, so windowing is arithmetic and
 * the graph gutter renders one SVG over the visible window plus overscan.
 * Edges span from a row's node center to the next row's node center so lane
 * lines stay continuous across rows; nodes paint on top and hide the joints.
 */
export function CommitGraphView({
    repoId,
    selectedId,
    onSelect,
    active = true,
}: {
    repoId: number;
    selectedId: string | null;
    onSelect: (commitId: string | null) => void;
    active?: boolean;
}) {
    const tabId = useTabId();
    const { state: session, controller } = useGraphSession({ repoId, tabId });
    const rows = useMemo(
        () => flattenRows(session.chunks, session.knownTotalRows),
        [session.chunks, session.knownTotalRows]
    );

    const containerRef = useRef<HTMLDivElement | null>(null);
    const [scrollTop, setScrollTop] = useState(0);
    const [viewportHeight, setViewportHeight] = useState(0);

    useEffect(() => {
        if (!active) return;
        const viewport = containerRef.current?.querySelector<HTMLDivElement>(
            '[data-slot="scroll-area-viewport"]'
        );
        if (!viewport) return;
        const onScroll = () => setScrollTop(viewport.scrollTop);
        viewport.addEventListener("scroll", onScroll);
        const observer = new ResizeObserver(() => {
            setScrollTop(viewport.scrollTop);
            setViewportHeight(viewport.clientHeight);
        });
        observer.observe(viewport);
        // Hidden tabs measure 0x0; re-measure when the tab becomes active.
        setViewportHeight(viewport.clientHeight);
        return () => {
            viewport.removeEventListener("scroll", onScroll);
            observer.disconnect();
        };
    }, [active, session.status]);

    const firstVisible = Math.max(
        0,
        Math.floor(scrollTop / GRAPH_ROW_HEIGHT_PX) - GRAPH_OVERSCAN_ROWS
    );
    const visibleCount =
        Math.ceil(viewportHeight / GRAPH_ROW_HEIGHT_PX) +
        GRAPH_OVERSCAN_ROWS * 2;
    const lastVisible = Math.min(rows.length, firstVisible + visibleCount);
    const windowRows = rows.slice(firstVisible, lastVisible);

    // Lanes over the full row list keep the gutter width stable while
    // scrolling; deep lanes only appear as the stream reaches them.
    const laneCount = useMemo(() => {
        let max = 1;
        for (const row of rows) {
            max = Math.max(max, row.lane + 1);
            for (const edge of row.edges) max = Math.max(max, edge.toLane + 1);
        }
        return max;
    }, [rows]);
    const gutterWidth = laneX(laneCount - 1) + LANE_PAD;

    // A completed session with a gap in the prefix catches up through a
    // range read; the backend keeps completed operations for range reads.
    useEffect(() => {
        if (session.status !== "completed") return;
        if (session.totalRows === null) return;
        if (rows.length >= session.totalRows) return;
        void controller.ensureRange(
            rows.length,
            Math.min(GRAPH_CATCHUP_ROWS, 50000)
        );
    }, [controller, session, rows.length]);

    if (session.status === "failed") {
        return (
            <p className="p-4 text-sm text-muted-foreground">
                {session.error?.message ?? "Failed to load history graph"}
            </p>
        );
    }
    if (
        session.status === "idle" ||
        (session.status === "running" && rows.length === 0)
    ) {
        return (
            <div>
                {Array.from({ length: 24 }).map((_, index) => (
                    <Skeleton
                        key={index}
                        className="w-full rounded-none"
                        style={{ height: GRAPH_ROW_HEIGHT_PX }}
                    />
                ))}
            </div>
        );
    }
    if (rows.length === 0) {
        return (
            <p
                className="p-4 text-center text-xs text-muted-foreground"
                data-slot="graph-empty"
            >
                No commits yet
            </p>
        );
    }

    return (
        <div
            ref={containerRef}
            className="size-full"
            data-slot="commit-graph-viewport"
        >
            <ScrollArea scrollX={false} overscrollContain>
                <div
                    className="relative"
                    style={{
                        height: Math.max(
                            rows.length * GRAPH_ROW_HEIGHT_PX,
                            viewportHeight
                        ),
                    }}
                >
                    {windowRows.map((row, offset) => (
                        <GraphRowLine
                            key={row.id}
                            row={row}
                            gutterWidth={gutterWidth}
                            top={(firstVisible + offset) * GRAPH_ROW_HEIGHT_PX}
                            selected={selectedId === row.id}
                            onSelect={onSelect}
                        />
                    ))}
                    <svg
                        aria-hidden="true"
                        className="pointer-events-none absolute left-0"
                        style={{
                            top: firstVisible * GRAPH_ROW_HEIGHT_PX,
                            width: gutterWidth,
                            // Edges of the last windowed row reach half a row
                            // past its center.
                            height:
                                windowRows.length * GRAPH_ROW_HEIGHT_PX +
                                GRAPH_ROW_HEIGHT_PX / 2,
                        }}
                    >
                        {windowRows.map((row, offset) => (
                            <GraphEdges
                                key={row.id}
                                row={row}
                                yCenter={
                                    offset * GRAPH_ROW_HEIGHT_PX +
                                    GRAPH_ROW_HEIGHT_PX / 2
                                }
                            />
                        ))}
                        {windowRows.map((row, offset) => (
                            <CommitNode
                                key={row.id}
                                row={row}
                                cy={
                                    offset * GRAPH_ROW_HEIGHT_PX +
                                    GRAPH_ROW_HEIGHT_PX / 2
                                }
                            />
                        ))}
                    </svg>
                </div>
            </ScrollArea>
        </div>
    );
}

function CommitNode({ row, cy }: { row: GraphRow; cy: number }) {
    const r = row.kind === "merge" ? MERGE_RADIUS : NODE_RADIUS;
    const cx = laneX(row.lane);
    return (
        <g>
            <circle
                cx={cx}
                cy={cy}
                r={r}
                fill="var(--background)"
                stroke={laneVar(row.lane)}
                strokeWidth={2}
            />
            <circle cx={cx} cy={cy} r={r - 4} fill={laneVar(row.lane)} />
        </g>
    );
}

function GraphEdges({ row, yCenter }: { row: GraphRow; yCenter: number }) {
    // Edges land on the next row's node center so segments join seamlessly.
    const yNext = yCenter + GRAPH_ROW_HEIGHT_PX;
    return (
        <g>
            {row.edges.map((edge, index) => {
                const fromX = laneX(edge.fromLane);
                const toX = laneX(edge.toLane);
                // Bends take the destination lane's color so the end of the
                // curve matches the node it lands on and the line below it.
                const stroke = laneVar(edge.toLane);
                if (edge.fromLane === edge.toLane) {
                    return (
                        <line
                            key={index}
                            x1={fromX}
                            y1={yCenter}
                            x2={fromX}
                            y2={yNext}
                            stroke={stroke}
                            strokeWidth={EDGE_STROKE}
                        />
                    );
                }
                // Drop, 90-degree turn through a horizontal run at the band
                // middle, then straight into the next node from above.
                const yMid = yCenter + GRAPH_ROW_HEIGHT_PX / 2;
                const dx = toX > fromX ? 1 : -1;
                const r = Math.min(
                    8,
                    Math.abs(toX - fromX) / 2,
                    GRAPH_ROW_HEIGHT_PX / 2 - 2
                );
                const d = [
                    `M ${fromX} ${yCenter}`,
                    `L ${fromX} ${yMid - r}`,
                    `Q ${fromX} ${yMid} ${fromX + dx * r} ${yMid}`,
                    `L ${toX - dx * r} ${yMid}`,
                    `Q ${toX} ${yMid} ${toX} ${yMid + r}`,
                    `L ${toX} ${yNext}`,
                ].join(" ");
                return (
                    <path
                        key={index}
                        d={d}
                        fill="none"
                        stroke={stroke}
                        strokeWidth={EDGE_STROKE}
                    />
                );
            })}
        </g>
    );
}

function GraphRowLine({
    row,
    gutterWidth,
    top,
    selected,
    onSelect,
}: {
    row: GraphRow;
    gutterWidth: number;
    top: number;
    selected: boolean;
    onSelect: (commitId: string) => void;
}) {
    const lane = row.lane;
    return (
        <div
            role="button"
            tabIndex={0}
            data-slot="graph-row"
            data-selected={selected || undefined}
            onClick={() => onSelect(row.id)}
            onKeyDown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    onSelect(row.id);
                }
            }}
            className="absolute inset-x-0 cursor-pointer [--tint:5%] hover:bg-accent hover:[--tint:9%] data-selected:bg-accent/64 data-selected:[--tint:13%]"
            style={
                {
                    top,
                    height: GRAPH_ROW_HEIGHT_PX,
                    "--row-lane": laneVar(lane),
                } as CSSProperties
            }
        >
            <div
                aria-hidden="true"
                className="pointer-events-none absolute inset-y-0 my-1 rounded-l-full border-r-2 border-dotted"
                style={{
                    left: REF_ZONE_WIDTH,
                    right: META_TOTAL,
                    background:
                        "color-mix(in srgb, var(--row-lane) var(--tint), transparent)",
                    borderColor: laneVar(lane),
                }}
            />
            {row.refs.length > 0 && (
                <div
                    className="absolute inset-y-0 flex min-w-0 items-center gap-1.5 overflow-hidden"
                    style={{ left: 12, width: laneX(lane) - 26 }}
                >
                    {row.refs.map((name) => (
                        <Badge
                            key={name}
                            className="min-w-0 shrink truncate"
                            style={{
                                background: laneAlpha(lane, 12),
                                color: laneVar(lane),
                            }}
                        >
                            {name}
                        </Badge>
                    ))}
                    <span
                        className="h-0 min-w-2 flex-1 border-t border-dashed"
                        style={{ borderColor: laneAlpha(lane, 45) }}
                    />
                </div>
            )}
            <div
                className="relative flex h-full min-w-0 items-center pr-4 text-sm"
                style={{ paddingLeft: gutterWidth + 12 }}
            >
                <span className="min-w-0 flex-1 truncate">
                    {row.summaryLine}
                </span>
                <div
                    className="flex h-full shrink-0 items-center justify-end gap-3 font-mono text-muted-foreground"
                    style={{ width: META_WIDTH }}
                >
                    <span className="hidden h-full min-w-0 flex-1 flex-col justify-center truncate border-r pr-2 text-center 2xl:flex">
                        {row.authorName}
                    </span>
                    <span className="flex h-full w-42 flex-col justify-center border-r pr-2 text-center">
                        {formatRelativeDate(row.timeSeconds * 1000)}
                    </span>
                    <span className="flex h-full shrink-0 flex-col justify-center">
                        {shortId(row.id)}
                    </span>
                </div>
            </div>
        </div>
    );
}
