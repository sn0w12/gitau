import {
    areaY,
    barY,
    crosshair,
    defineChart,
    lineY,
    type ChartCurve,
    type ChartMotionContext,
    type ChartMotionTiming,
} from "@tanstack/charts";
import { motion } from "@tanstack/charts/motion";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { tooltip } from "@tanstack/charts/tooltip";
import { RendererChart } from "@tanstack/react-charts/tooltip";
import {
    useId,
    useLayoutEffect,
    useMemo,
    useRef,
    useState,
    type CSSProperties,
} from "react";

import {
    HISTORY_CHART_HEIGHT,
    type HistoryChartRow,
} from "@/components/repo/history/history-chart-types";

export { HISTORY_CHART_HEIGHT };
export type { HistoryChartRow };

const BAR_RADIUS = 4;
const BAR_GROW_MS = 350;
const BAR_WAVE_SPREAD_MS = 900;

const SERIES = [
    { field: "additions", label: "Additions", color: "var(--success)" },
    { field: "deletions", label: "Deletions", color: "var(--destructive)" },
    { field: "commits", label: "Commits", color: "var(--foreground)" },
] as const;

function cubicBezier(x1: number, y1: number, x2: number, y2: number) {
    const value = (a: number, b: number, t: number) =>
        3 * a * t + (3 * b - 6 * a) * t * t + (1 - 3 * b + 3 * a) * t * t * t;
    const slope = (a: number, b: number, t: number) =>
        3 * a + 2 * (3 * b - 6 * a) * t + 3 * (1 - 3 * b + 3 * a) * t * t;

    return (progress: number) => {
        if (progress <= 0 || progress >= 1) return progress;
        let guess = progress;
        for (let i = 0; i < 12; i++) {
            const error = value(x1, x2, guess) - progress;
            if (Math.abs(error) < 1e-6) break;
            const derivative = slope(x1, x2, guess);
            if (Math.abs(derivative) < 1e-6) break;
            guess = Math.min(1, Math.max(0, guess - error / derivative));
        }
        // The renderer multiplies painted geometry by this, so an unclamped
        // solve puts bars and areas outside the plot.
        return Math.min(1, Math.max(0, value(y1, y2, guess)));
    };
}

export const revealEase = cubicBezier(0, 0.7, 0.5, 1);

function monotoneLine(points: readonly (readonly [number, number])[]): string {
    if (points.length === 0) return "";
    if (points.length === 1) return `M${points[0][0]},${points[0][1]}`;

    const last = points.length - 1;
    if (last === 1) {
        return `M${points[0][0]},${points[0][1]}L${points[1][0]},${points[1][1]}`;
    }

    const slope: number[] = [];
    for (let i = 0; i < last; i++) {
        const dx = points[i + 1][0] - points[i][0];
        slope[i] = dx === 0 ? 0 : (points[i + 1][1] - points[i][1]) / dx;
    }

    const tangent: number[] = [];
    tangent[0] = slope[0];
    tangent[last] = slope[last - 1];
    for (let i = 1; i < last; i++) {
        const before = slope[i - 1];
        const after = slope[i];
        if (before * after <= 0) {
            tangent[i] = 0;
            continue;
        }
        const mean = 2 / (1 / before + 1 / after);
        const limit = 3 * Math.min(Math.abs(before), Math.abs(after));
        tangent[i] = Math.sign(mean) * Math.min(Math.abs(mean), limit);
    }

    let d = `M${points[0][0]},${points[0][1]}`;
    for (let i = 0; i < last; i++) {
        const [x0, y0] = points[i];
        const [x1, y1] = points[i + 1];
        const step = (x1 - x0) / 3;
        d += `C${x0 + step},${y0 + tangent[i] * step} ${x1 - step},${
            y1 - tangent[i + 1] * step
        } ${x1},${y1}`;
    }
    return d;
}

const monotoneCurve: ChartCurve = {
    line: monotoneLine,
    area(top, bottom) {
        if (top.length === 0) return "";
        const closing = [...bottom]
            .reverse()
            .map(([x, y]) => `L${x},${y}`)
            .join("");
        return `${monotoneLine(top)}${closing}Z`;
    },
};

// Without this a refetch resolves an `update` phase and re-animates every bar.
function entrance<TDatum>(
    build: (context: ChartMotionContext<TDatum>) => ChartMotionTiming<TDatum>
): (context: ChartMotionContext<TDatum>) => ChartMotionTiming<TDatum> | false {
    return (context) => (context.phase === "enter" ? build(context) : false);
}

export function HistoryChart({ rows }: { rows: readonly HistoryChartRow[] }) {
    const uid = useId().replace(/[^a-zA-Z0-9-]/g, "");
    const gradientId = `history-commits-${uid}`;
    const revealId = `history-reveal-${uid}`;

    const renderer = useMemo(
        () => motion<HistoryChartRow, string, number>(),
        []
    );

    const definition = useMemo(() => {
        // A fixed budget keeps a dense chart's entrance as quick as a sparse one.
        const lastIndex = rows.length - 1;
        const perBarDelay = lastIndex > 0 ? BAR_WAVE_SPREAD_MS / lastIndex : 0;
        const barGrow: ChartMotionTiming<HistoryChartRow> = {
            transition: {
                type: "tween",
                duration: BAR_GROW_MS,
                easing: revealEase,
            },
        };
        const growBar = (phaseShift: number) =>
            entrance((context) => ({
                ...barGrow,
                delay: context.datumIndex * perBarDelay + phaseShift,
            }));

        return defineChart(
            {
                marks: [
                    // `focusGuideOnly`, so hovering repaints one keyed element
                    // rather than the scene `paintFocus` re-serializes.
                    crosshair({
                        x: {
                            band: {
                                radius: 3,
                                fill: "var(--foreground)",
                                fillOpacity: 0.06,
                            },
                        },
                        y: false,
                        motion: false,
                    }),
                    // Mark order is z order; the trace sits behind the bars.
                    areaY(rows, {
                        x: "label",
                        y1: 0,
                        y2: "commits",
                        yScale: "commits",
                        fill: `url(#${gradientId})`,
                        curve: monotoneCurve,
                        motion: false,
                    }),
                    lineY(rows, {
                        x: "label",
                        y: "commits",
                        yScale: "commits",
                        stroke: "var(--foreground)",
                        strokeWidth: 1.5,
                        curve: monotoneCurve,
                        motion: false,
                    }),
                    barY(rows, {
                        x: "label",
                        y1: 0,
                        y2: "additions",
                        fill: "var(--success)",
                        radius: BAR_RADIUS,
                        motion: growBar(0),
                    }),
                    barY(rows, {
                        x: "label",
                        y1: "additions",
                        y2: (row: HistoryChartRow) =>
                            row.additions + row.deletions,
                        fill: "var(--destructive)",
                        radius: BAR_RADIUS,
                        motion: growBar(BAR_GROW_MS),
                    }),
                ],
                scales: {
                    x: {
                        scale: () => scaleBand<string>().padding(0),
                        axis: false,
                    },
                    y: { scale: scaleLinear, axis: false },
                    commits: {
                        channel: "y",
                        scale: scaleLinear,
                        side: "right",
                        axis: false,
                    },
                },
                gradients: [
                    {
                        id: gradientId,
                        // The library's linear default runs bottom-to-top.
                        y1: 0,
                        y2: 1,
                        stops: [
                            {
                                offset: 0,
                                color: "var(--foreground)",
                                opacity: 0.32,
                            },
                            {
                                offset: 1,
                                color: "var(--foreground)",
                                opacity: 0,
                            },
                        ],
                    },
                ],
                margin: 0,
                clip: true,
            },
            {
                focus: "group-x",
                maxFocusDistance: Number.POSITIVE_INFINITY,
                focusRing: false,
                tooltip: {
                    use: tooltip,
                    anchor: "pointer",
                    offset: 12,
                    motion: false,
                },
            }
        );
    }, [gradientId, rows]);

    return (
        <div
            className="size-full"
            style={
                {
                    "--history-chart-reveal": `url(#${revealId})`,
                } as CSSProperties
            }
        >
            <svg
                width="0"
                height="0"
                className="absolute"
                aria-hidden="true"
                focusable="false"
            >
                <defs>
                    <clipPath id={revealId} clipPathUnits="objectBoundingBox">
                        <rect
                            x="0"
                            y="0"
                            width="1"
                            height="1"
                            className="history-chart__reveal"
                        />
                    </clipPath>
                </defs>
            </svg>
            <RendererChart
                className="history-chart size-full"
                height={HISTORY_CHART_HEIGHT}
                definition={definition}
                renderer={renderer}
                ariaLabel="Commits and line changes per time bucket"
                renderTooltipBody={({ points }) => (
                    <HistoryTooltip row={points[0]?.datum} />
                )}
            />
        </div>
    );
}

// A custom tooltip body makes the host strip its own card to transparent,
// so the whole surface has to be composed here.
function HistoryTooltip({ row }: { row: HistoryChartRow | undefined }) {
    if (!row) return null;

    return <HistoryTooltipCard row={row} />;
}

function HistoryTooltipCard({ row }: { row: HistoryChartRow }) {
    const contentRef = useRef<HTMLDivElement>(null);
    const [width, setWidth] = useState<number>();

    useLayoutEffect(() => {
        const content = contentRef.current;
        if (!content) return;
        const measured = content.getBoundingClientRect().width;
        if (measured > 0) setWidth(measured);
    }, [row]);

    return (
        <div
            className="overflow-hidden rounded-lg border border-border/50 bg-background shadow-xl transition-[width] duration-150 ease-snappy"
            style={width === undefined ? undefined : { width }}
        >
            <div
                ref={contentRef}
                className="grid w-max min-w-32 items-start gap-1 px-2.5 py-1.5 text-xs text-foreground"
            >
                <div className="font-medium">{row.label}</div>
                <div className="grid gap-0.5">
                    {SERIES.map((series) => (
                        <div
                            key={series.field}
                            className="flex items-center justify-between gap-4"
                        >
                            <div className="flex items-center gap-2">
                                <span
                                    aria-hidden="true"
                                    className="size-2.5 shrink-0 rounded-xs"
                                    style={{ background: series.color }}
                                />
                                <span className="text-muted-foreground">
                                    {series.label}
                                </span>
                            </div>
                            <span className="font-mono font-medium tabular-nums">
                                {row[series.field].toLocaleString()}
                            </span>
                        </div>
                    ))}
                </div>
            </div>
        </div>
    );
}
