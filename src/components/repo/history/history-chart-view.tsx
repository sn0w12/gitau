import { useMemo } from "react";

import {
    HistoryChart,
    HISTORY_CHART_HEIGHT,
    type HistoryChartRow,
} from "@/components/repo/history/history-chart";
import { useHistoryChart } from "@/hooks/repositories/use-repository-queries";
import type { ChartBucketSize } from "@/lib/backend/protocol";

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

export function HistoryChartView({ repoId }: { repoId: number }) {
    const chart = useHistoryChart(repoId);
    const data = chart.data;

    const rows = useMemo<HistoryChartRow[]>(() => {
        if (!data || data.buckets.length === 0) return [];
        const tick = createTickFormatter(data.bucketSize);
        return data.buckets.map((bucket) => ({
            label: tick(bucket.startSeconds),
            additions: bucket.additions,
            deletions: bucket.deletions,
            commits: bucket.commits,
        }));
    }, [data]);

    if (chart.isError) {
        return (
            <div
                style={{ height: HISTORY_CHART_HEIGHT }}
                className="flex items-center justify-center p-4 text-sm text-muted-foreground"
            >
                {chart.error.message}
            </div>
        );
    } else if (!chart.isPending && rows.length === 0) {
        return null;
    }

    return (
        <div
            style={{ height: HISTORY_CHART_HEIGHT }}
            className="relative w-full border-b"
        >
            {rows.length > 0 && <HistoryChart rows={rows} />}
            {chart.isPending && (
                <div className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center">
                    <div className="flex items-center justify-center gap-2 rounded-md border bg-background px-2 py-0.5 text-sm text-primary">
                        <div className="h-3 w-3 animate-spin rounded-full border border-border border-t-primary" />
                        <span>Loading</span>
                    </div>
                </div>
            )}
        </div>
    );
}
