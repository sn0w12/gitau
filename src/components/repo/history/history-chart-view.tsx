import { useMemo } from "react";

import { EvilComposedChart } from "@/components/evilcharts/charts/recharts-composed-chart";
import type { ChartConfig } from "@/components/evilcharts/ui/recharts-chart";
import { useHistoryChart } from "@/hooks/repositories/use-repository-queries";
import type { ChartBucketSize } from "@/lib/backend/protocol";

const chartConfig = {
    additions: {
        label: "Additions",
        colors: { light: ["var(--success)"], dark: ["var(--success)"] },
    },
    deletions: {
        label: "Deletions",
        colors: { light: ["var(--destructive)"], dark: ["var(--destructive)"] },
    },
    commits: {
        label: "Commits",
        colors: { light: ["var(--foreground)"], dark: ["var(--foreground)"] },
    },
} satisfies ChartConfig;

/** UTC-only formatters: bucket starts are epoch-aligned to UTC boundaries. */
function createTickFormatter(bucketSize: ChartBucketSize) {
    const date = new Intl.DateTimeFormat("en-US", {
        timeZone: "UTC",
        month: "short",
        day: "numeric",
    });
    const dateTime = new Intl.DateTimeFormat("en-US", {
        timeZone: "UTC",
        month: "short",
        day: "numeric",
        hour: "numeric",
    });
    const monthYear = new Intl.DateTimeFormat("en-US", {
        timeZone: "UTC",
        month: "short",
        year: "numeric",
    });

    return (startSeconds: number) => {
        const start = new Date(startSeconds * 1000);
        switch (bucketSize) {
            // Sub-day tiers need the time of day to stay unambiguous.
            case "hour":
            case "quarterDay":
            case "halfDay":
                return dateTime.format(start);
            case "month":
                return monthYear.format(start);
            case "quarter": {
                const month = start.getUTCMonth();
                return `Q${Math.floor(month / 3) + 1} ${start.getUTCFullYear()}`;
            }
            default:
                return date.format(start);
        }
    };
}

// Deletions start exactly one grow after additions so the stack builds
// bottom-up.
const BAR_GROW_SECONDS = 0.35;

export function HistoryChartView({ repoId }: { repoId: number }) {
    const chart = useHistoryChart(repoId);
    const data = chart.data;

    const rows = useMemo(() => {
        if (!data || data.buckets.length === 0) return [];
        const tick = createTickFormatter(data.bucketSize);
        return data.buckets.map((bucket) => ({
            ...bucket,
            label: tick(bucket.startSeconds),
        }));
    }, [data]);

    if (chart.isError) {
        return (
            <div className="flex h-[calc(var(--spacing)*18.5-1px)] items-center justify-center p-4 text-sm text-muted-foreground">
                {chart.error.message}
            </div>
        );
    } else if (!chart.isPending && rows.length === 0) {
        return null;
    }

    return (
        <div className="relative h-[calc(var(--spacing)*18.5-1px)] w-full border-b">
            <svg
                width="0"
                height="0"
                className="absolute"
                aria-hidden="true"
                focusable="false"
            >
                <defs>
                    <linearGradient
                        id={`history-chart-${repoId}-commits-fill`}
                        x1="0"
                        y1="0"
                        x2="0"
                        y2="1"
                    >
                        <stop
                            offset="0%"
                            stopColor="var(--foreground)"
                            stopOpacity={0.32}
                        />
                        <stop
                            offset="100%"
                            stopColor="var(--foreground)"
                            stopOpacity={0}
                        />
                    </linearGradient>
                </defs>
            </svg>
            <EvilComposedChart
                className="size-full"
                config={chartConfig}
                data={rows}
                isLoading={chart.isPending}
                xDataKey="label"
                barCategoryGap={0}
                chartProps={{
                    margin: { top: 0, right: 0, bottom: 0, left: 0 },
                    // Non-interactive decoration: kill the recharts
                    // accessibilityLayer's tabbable svg so clicks never show a
                    // focus ring.
                    tabIndex: -1,
                }}
            >
                <EvilComposedChart.Bar
                    dataKey="additions"
                    barProps={{ yAxisId: "lines", stackId: "diff" }}
                    enableHoverHighlight
                    growDuration={BAR_GROW_SECONDS}
                    isClickable={false}
                />
                <EvilComposedChart.Bar
                    dataKey="deletions"
                    barProps={{ yAxisId: "lines", stackId: "diff" }}
                    enableHoverHighlight
                    growDuration={BAR_GROW_SECONDS}
                    growDelay={BAR_GROW_SECONDS}
                    isClickable={false}
                />
                <EvilComposedChart.XAxis dataKey="label" hide />
                <EvilComposedChart.YAxis yAxisId="lines" hide />
                <EvilComposedChart.YAxis
                    yAxisId="commits"
                    orientation="right"
                    hide
                />
                <EvilComposedChart.Area
                    dataKey="commits"
                    areaProps={{
                        dataKey: "commits",
                        yAxisId: "commits",
                        stroke: "var(--foreground)",
                        strokeWidth: 1.5,
                        fill: `url(#history-chart-${repoId}-commits-fill)`,
                    }}
                    curveType="monotone"
                    strokeVariant="solid"
                    isClickable={false}
                />
                <EvilComposedChart.Tooltip />
            </EvilComposedChart>
        </div>
    );
}
