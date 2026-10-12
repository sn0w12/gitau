import {
    areaY,
    barY,
    crosshair,
    defineChart,
    lineY,
    type ChartMarkState,
} from "@tanstack/charts";
import { scaleBand } from "@tanstack/charts/scales/band";
import { scaleLinear } from "@tanstack/charts/scales/linear";
import { renderChartSvg } from "@tanstack/charts/svg";
import { createSvgChartRenderer } from "@tanstack/charts/svg/renderer";
// @vitest-environment jsdom
import { Chart } from "@tanstack/react-charts/core";
import { act, useMemo } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it, vi } from "vitest";

import { HistoryChart } from "@/components/repo/history/history-chart";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

class FakeResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
}
vi.stubGlobal("ResizeObserver", FakeResizeObserver);

type Row = {
    label: string;
    additions: number;
    deletions: number;
    commits: number;
};

const FOCUS_CHANGES = 20;

function makeRows(n: number): Row[] {
    return Array.from({ length: n }, (_, i) => ({
        label: `Day ${i}`,
        additions: Math.round(20 + 90 * Math.abs(Math.sin(i / 3))),
        deletions: Math.round(10 + 60 * Math.abs(Math.cos(i / 4))),
        commits: 1 + Math.round(20 * Math.abs(Math.sin(i / 5))),
    }));
}

const dimStates: ChartMarkState<Row>[] = [
    {
        when: (context) => context.matches("x"),
        style: { opacity: 1 },
    },
    {
        when: (context) => !context.matches("x"),
        style: { opacity: 0.3 },
    },
];

function Probe({
    rows,
    states,
    onRender,
}: {
    rows: Row[];
    states?: ChartMarkState<Row>[];
    onRender: (context: unknown) => void;
}) {
    const definition = useMemo(
        () =>
            defineChart(
                {
                    marks: [
                        crosshair({
                            x: {
                                band: {
                                    radius: 3,
                                    fill: "var(--foreground)",
                                },
                            },
                            y: false,
                        }),
                        areaY(rows, {
                            x: "label",
                            y1: 0,
                            y2: "commits",
                            yScale: "commits",
                            fill: "var(--foreground)",
                            fillOpacity: 0.15,
                            motion: false,
                        }),
                        lineY(rows, {
                            x: "label",
                            y: "commits",
                            yScale: "commits",
                            stroke: "var(--foreground)",
                            motion: false,
                        }),
                        barY(rows, {
                            x: "label",
                            y1: 0,
                            y2: "additions",
                            fill: "var(--success)",
                            motion: false,
                            states,
                        }),
                        barY(rows, {
                            x: "label",
                            y1: "additions",
                            y2: (row: Row) => row.additions + row.deletions,
                            fill: "var(--destructive)",
                            motion: false,
                            states,
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
                    margin: 0,
                    clip: true,
                },
                {
                    focus: "group-x",
                    maxFocusDistance: Number.POSITIVE_INFINITY,
                    focusRing: false,
                    pointer: false,
                }
            ) as never,
        [rows, states]
    );
    const renderer = useMemo(
        () => createSvgChartRenderer(renderChartSvg) as never,
        []
    );
    return (
        <Chart
            className="size-full"
            height={73}
            definition={definition}
            renderer={renderer}
            ariaLabel="probe"
            onRender={onRender}
        />
    );
}

async function mutationsPerFocusChange(
    states: ChartMarkState<Row>[] | undefined,
    buckets: number
) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    const rows = makeRows(buckets);
    // The probe's erased generics leave `onRender`'s context untyped.
    const captured: {
        interaction: {
            setControlledFocus: (point: unknown, options?: unknown) => void;
        };
        scene: { points: { key: string }[] };
    }[] = [];
    const onRender = (value: unknown) => {
        captured.push(value as never);
    };

    await act(async () => {
        root.render(<Probe rows={rows} states={states} onRender={onRender} />);
    });
    await act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 300));
    });

    const svg = container.querySelector("svg.ts-chart") as SVGSVGElement;
    const context = captured[captured.length - 1];
    const bars = context.scene.points.filter((point) =>
        String(point.key).startsWith("bar-y")
    );
    expect(bars.length).toBeGreaterThan(0);

    let mutations = 0;
    const observer = new MutationObserver((records) => {
        mutations += records.length;
    });
    observer.observe(svg, { attributes: true, childList: true, subtree: true });

    for (const point of bars.slice(0, FOCUS_CHANGES)) {
        await act(async () => {
            context.interaction.setControlledFocus(point, {
                source: "programmatic",
            });
            await Promise.resolve();
        });
    }
    observer.disconnect();
    act(() => root.unmount());
    container.remove();
    return mutations / FOCUS_CHANGES;
}

describe("hover cost", () => {
    it("per-bar states make every focus change rewrite the whole bar series", async () => {
        const withStates = await mutationsPerFocusChange(dimStates, 600);
        const withoutStates = await mutationsPerFocusChange(undefined, 600);

        // `states` resolve into a new scene, latching `markStatePainted` in
        // svg-surface and forcing the full-scene serialize/parse/diff.
        expect(withStates).toBeGreaterThan(10);
        expect(withoutStates).toBeLessThanOrEqual(2);
        expect(withStates / withoutStates).toBeGreaterThan(5);
    }, 180000);

    it("the shipped chart declares no per-bar mark states", async () => {
        const container = document.createElement("div");
        document.body.appendChild(container);
        const root = createRoot(container);
        await act(async () => {
            root.render(<HistoryChart rows={makeRows(30)} />);
        });
        await act(async () => {
            await new Promise((resolve) => setTimeout(resolve, 300));
        });

        expect(
            container.querySelectorAll("g[data-ts-key^='states:']")
        ).toHaveLength(0);

        act(() => root.unmount());
        container.remove();
    }, 60000);
});
