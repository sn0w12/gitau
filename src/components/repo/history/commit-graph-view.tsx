import { useEffect, useId, useMemo, useRef, useState } from "react";
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
const LANE_GAP = 16;
const LANE_PAD = 14;
/** Lanes past this clip out of the rail so it stays a small share of a row. */
const MAX_LANE_COLUMNS = 8;
const EDGE_STROKE = 2;
/**
 * Share of a connector spent blending from one lane color into the next. The
 * blend sits in the middle, so the connector starts in the lane it leaves and
 * ends in the lane it joins.
 */
const LANE_BLEND_FRACTION = 0.15;
const NODE_RADIUS = 6;
const MERGE_RADIUS = 7;
const COLUMN_COUNT = 6;

export function laneVar(lane: number): string {
    return `var(--graph-lane-${((lane % LANE_COLORS) + LANE_COLORS) % LANE_COLORS})`;
}

function laneX(lane: number): number {
    return LANE_PAD + lane * LANE_GAP;
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

/**
 * The backend decorates a commit with every ref that peels to it, so a pushed
 * branch arrives twice: `feature/x` and `origin/feature/x`. Refs sharing a
 * trailing path segment are the same branch seen through a remote, and the
 * shallowest name wins (`feature/x`, not `origin/feature/x`).
 */
export function collapseRefs(refs: string[]): string[] {
    const shallowest = new Map<string, { ref: string; depth: number }>();
    for (const ref of refs) {
        const slash = ref.lastIndexOf("/");
        const tail = slash === -1 ? ref : ref.slice(slash + 1);
        const depth = slash === -1 ? 0 : ref.split("/").length;
        const seen = shallowest.get(tail);
        if (seen === undefined || depth < seen.depth) {
            shallowest.set(tail, { ref, depth });
        }
    }
    return [...shallowest.values()].map((entry) => entry.ref);
}

function shortId(id: string): string {
    return id.slice(0, 7);
}

/**
 * Headerless commit table: rail, summary, refs, author, date, short id.
 * Rows have a fixed height, so the window is picked arithmetically and the
 * rows above and below it are replaced by one spacer row each. Spacers rather
 * than absolutely positioned rows keep real rows in normal flow, so
 * `table-fixed` and the colgroup hold the column widths across scrolls.
 *
 * Column widths are `min(<content size>, <share of the table>)`. Fixed table
 * layout never shrinks a specified width, so an uncapped column would push the
 * summary off a narrow panel instead of giving up its own space.
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

    // Lanes over the full row list keep the rail width stable while
    // scrolling; deep lanes only appear as the stream reaches them.
    const railWidth = useMemo(() => {
        let deepest = 1;
        for (const row of rows) {
            deepest = Math.max(deepest, row.lane + 1);
            for (const edge of row.edges)
                deepest = Math.max(deepest, edge.toLane + 1);
        }
        return (
            LANE_PAD * 2 + (Math.min(deepest, MAX_LANE_COLUMNS) - 1) * LANE_GAP
        );
    }, [rows]);

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
                    className="relative overflow-hidden pr-2"
                    style={{
                        minHeight: Math.max(
                            rows.length * GRAPH_ROW_HEIGHT_PX,
                            viewportHeight
                        ),
                    }}
                >
                    <table
                        className="w-full table-fixed border-separate border-spacing-0 text-sm whitespace-nowrap"
                        style={{ "--rail": `${railWidth}px` } as CSSProperties}
                    >
                        <colgroup>
                            <col style={{ width: "var(--rail)" }} />
                            <col />
                            <col
                                style={{ width: "min(calc(18ch + 1rem), 16%)" }}
                            />
                            <col
                                style={{ width: "min(calc(14ch + 1rem), 12%)" }}
                            />
                            <col style={{ width: "calc(13ch + 1rem)" }} />
                            <col
                                className="font-mono"
                                style={{ width: "calc(7ch + 1rem)" }}
                            />
                        </colgroup>
                        <tbody>
                            <SpacerRow
                                height={firstVisible * GRAPH_ROW_HEIGHT_PX}
                            />
                            {windowRows.map((row) => (
                                <CommitGraphRow
                                    key={row.id}
                                    row={row}
                                    selected={selectedId === row.id}
                                    onSelect={onSelect}
                                />
                            ))}
                            <SpacerRow
                                height={
                                    (rows.length - lastVisible) *
                                    GRAPH_ROW_HEIGHT_PX
                                }
                            />
                        </tbody>
                    </table>
                    <svg
                        aria-hidden="true"
                        className="pointer-events-none absolute left-0"
                        style={{
                            top: firstVisible * GRAPH_ROW_HEIGHT_PX,
                            width: railWidth,
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

function SpacerRow({ height }: { height: number }) {
    if (height <= 0) return null;
    return (
        <tr aria-hidden="true" data-slot="graph-spacer">
            <td colSpan={COLUMN_COUNT} className="p-0" style={{ height }} />
        </tr>
    );
}

function CommitGraphRow({
    row,
    selected,
    onSelect,
}: {
    row: GraphRow;
    selected: boolean;
    onSelect: (commitId: string) => void;
}) {
    const lane = row.lane;
    const refs = collapseRefs(row.refs);
    return (
        <tr
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
            className="cursor-pointer hover:bg-accent/40 focus-visible:bg-accent/64 focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-ring data-selected:bg-accent/64"
            style={{ height: GRAPH_ROW_HEIGHT_PX }}
        >
            <td className="p-0" />
            <td className="truncate px-2">{row.summaryLine}</td>
            <td className="truncate px-2">
                {refs.length > 0 && (
                    <span className="flex min-w-0 items-center gap-1">
                        {refs.map((name) => (
                            <Badge
                                key={name}
                                className="max-w-1/2 min-w-0 shrink"
                                style={{
                                    background: laneAlpha(lane, 12),
                                    color: laneVar(lane),
                                }}
                            >
                                <span className="min-w-0 truncate">{name}</span>
                            </Badge>
                        ))}
                    </span>
                )}
            </td>
            <td className="truncate px-2 text-muted-foreground">
                {row.authorName}
            </td>
            <td className="truncate px-2 text-muted-foreground">
                {formatRelativeDate(row.timeSeconds * 1000)}
            </td>
            <td className="truncate px-2 font-mono text-muted-foreground">
                {shortId(row.id)}
            </td>
        </tr>
    );
}

function CommitNode({ row, cy }: { row: GraphRow; cy: number }) {
    const r = row.kind === "merge" ? MERGE_RADIUS : NODE_RADIUS;
    return (
        <g>
            {/* Opaque so the rail is hidden inside the ring and meets it at
                the edge rather than showing through the node. */}
            <circle
                cx={laneX(row.lane)}
                cy={cy}
                r={r}
                fill="var(--background)"
                stroke={laneVar(row.lane)}
                strokeWidth={2}
            />
            <circle
                cx={laneX(row.lane)}
                cy={cy}
                r={r - 4}
                fill={laneVar(row.lane)}
            />
        </g>
    );
}

function GraphEdges({ row, yCenter }: { row: GraphRow; yCenter: number }) {
    // Ids reach the document through url(#id), which rejects the characters
    // useId may emit, so keep only the identifier-safe ones.
    const prefix = useId().replace(/[^a-zA-Z0-9]/g, "");
    const yNext = yCenter + GRAPH_ROW_HEIGHT_PX;
    const blendLength = GRAPH_ROW_HEIGHT_PX * LANE_BLEND_FRACTION;
    const blendStart = yCenter + (GRAPH_ROW_HEIGHT_PX - blendLength) / 2;
    const blendEnd = blendStart + blendLength;
    const shifts = row.edges
        .map((edge, index) => ({ edge, index }))
        .filter(({ edge }) => laneVar(edge.fromLane) !== laneVar(edge.toLane));
    return (
        <g>
            {shifts.length > 0 && (
                <defs>
                    {shifts.map(({ edge, index }) => {
                        const toX = laneX(edge.toLane);
                        return (
                            <linearGradient
                                key={index}
                                id={`${prefix}-${index}`}
                                gradientUnits="userSpaceOnUse"
                                x1={toX}
                                y1={blendStart}
                                x2={toX}
                                y2={blendEnd}
                            >
                                <stop
                                    offset="0"
                                    style={{
                                        stopColor: laneVar(edge.fromLane),
                                    }}
                                />
                                <stop
                                    offset="0.5"
                                    style={{
                                        stopColor: `color-mix(in oklch, ${laneVar(
                                            edge.fromLane
                                        )} 50%, ${laneVar(edge.toLane)})`,
                                    }}
                                />
                                <stop
                                    offset="1"
                                    style={{
                                        stopColor: laneVar(edge.toLane),
                                    }}
                                />
                            </linearGradient>
                        );
                    })}
                </defs>
            )}
            {row.edges.map((edge, index) => {
                const fromX = laneX(edge.fromLane);
                const toX = laneX(edge.toLane);
                // Same-lane edges have nothing to hand over, and skipping the
                // paint server keeps through-lanes off the gradient path.
                const stroke =
                    laneVar(edge.fromLane) === laneVar(edge.toLane)
                        ? laneVar(edge.toLane)
                        : `url(#${prefix}-${index})`;
                if (fromX === toX) {
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
                return (
                    <path
                        key={index}
                        d={[
                            `M ${fromX} ${yCenter}`,
                            `L ${fromX} ${yMid - r}`,
                            `Q ${fromX} ${yMid} ${fromX + dx * r} ${yMid}`,
                            `L ${toX - dx * r} ${yMid}`,
                            `Q ${toX} ${yMid} ${toX} ${yMid + r}`,
                            `L ${toX} ${yNext}`,
                        ].join(" ")}
                        fill="none"
                        stroke={stroke}
                        strokeWidth={EDGE_STROKE}
                    />
                );
            })}
        </g>
    );
}
