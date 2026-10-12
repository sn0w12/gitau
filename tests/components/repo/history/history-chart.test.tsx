// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
    HistoryChart,
    HISTORY_CHART_HEIGHT,
    revealEase,
} from "@/components/repo/history/history-chart";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

class FakeResizeObserver {
    observe() {}
    unobserve() {}
    disconnect() {}
}

vi.stubGlobal("ResizeObserver", FakeResizeObserver);

const rows = [
    { label: "Jan 1", additions: 40, deletions: 10, commits: 3 },
    { label: "Jan 2", additions: 90, deletions: 25, commits: 7 },
    { label: "Jan 3", additions: 15, deletions: 60, commits: 1 },
    { label: "Jan 4", additions: 70, deletions: 5, commits: 12 },
];

// The renderer strips its per-mark attributes once the entrance settles, so
// geometry assertions have to wait for it.
function mount() {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(<HistoryChart rows={rows} />);
    });
    return {
        container,
        settle: async () => {
            await act(async () => {
                await new Promise((resolve) => setTimeout(resolve, 2500));
            });
        },
        unmount() {
            act(() => root.unmount());
            container.remove();
        },
    };
}

function marks(container: HTMLElement, fill: string) {
    return [...container.querySelectorAll("rect")].filter(
        (rect) => rect.getAttribute("fill") === fill
    );
}

function geometry(rect: Element) {
    return {
        x: Number(rect.getAttribute("x")),
        y: Number(rect.getAttribute("y")),
        width: Number(rect.getAttribute("width")),
        height: Number(rect.getAttribute("height")),
    };
}

describe("HistoryChart", () => {
    beforeEach(() => {
        document.body.innerHTML = "";
    });

    it("eases only within the unit interval and only upward", () => {
        const samples: number[] = [];
        for (let i = 0; i <= 2000; i++) {
            samples.push(revealEase(i / 2000));
        }

        for (const value of samples) {
            expect(Number.isFinite(value)).toBe(true);
            expect(value).toBeGreaterThanOrEqual(0);
            expect(value).toBeLessThanOrEqual(1);
        }

        expect(samples[0]).toBe(0);
        expect(samples[samples.length - 1]).toBe(1);
        for (const [index, value] of samples.entries()) {
            expect(value).toBeGreaterThanOrEqual(samples[index - 1] ?? 0);
        }

        expect(revealEase(0.5)).toBeGreaterThan(0.5);
    });

    it("paints one bar per series per bucket", () => {
        const { container, unmount } = mount();

        expect(marks(container, "var(--success)")).toHaveLength(rows.length);
        expect(marks(container, "var(--destructive)")).toHaveLength(
            rows.length
        );

        unmount();
    });

    it("stacks deletions on top of additions within each column", async () => {
        const { container, settle, unmount } = mount();
        await settle();

        const additions = marks(container, "var(--success)").map(geometry);
        const deletions = marks(container, "var(--destructive)").map(geometry);

        for (const [index] of rows.entries()) {
            const bottom = additions[index];
            const top = deletions[index];

            expect(top.x).toBe(bottom.x);
            expect(top.width).toBe(bottom.width);
            expect(bottom.y + bottom.height).toBeCloseTo(
                HISTORY_CHART_HEIGHT,
                1
            );
            expect(top.y + top.height).toBeCloseTo(bottom.y, 1);
        }

        unmount();
    });

    it("scales the stack so the busiest bucket fills the plot", async () => {
        const { container, settle, unmount } = mount();
        await settle();

        const stackHeights = rows.map((_, index) => {
            const additions = geometry(
                marks(container, "var(--success)")[index]
            );
            const deletions = geometry(
                marks(container, "var(--destructive)")[index]
            );
            return additions.height + deletions.height;
        });

        // Jan 2 has the most line changes (90 + 25).
        const busiest = rows.reduce(
            (best, row, index) =>
                row.additions + row.deletions >
                rows[best].additions + rows[best].deletions
                    ? index
                    : best,
            0
        );
        expect(stackHeights[busiest]).toBeCloseTo(HISTORY_CHART_HEIGHT, 1);
        for (const [index, height] of stackHeights.entries()) {
            if (index !== busiest)
                expect(height).toBeLessThan(HISTORY_CHART_HEIGHT);
        }

        unmount();
    });

    it("fills the width with edge-to-edge columns", async () => {
        const { container, settle, unmount } = mount();
        await settle();

        const additions = marks(container, "var(--success)").map(geometry);

        expect(additions[0].x).toBe(0);
        for (const [index, bar] of additions.entries()) {
            expect(bar.width).toBe(additions[0].width);
            if (index > 0) {
                expect(bar.x).toBe(additions[index - 1].x + bar.width);
            }
        }

        unmount();
    });

    it("fills the commits area from the baseline", () => {
        const { container, unmount } = mount();

        const area = container.querySelector("path[fill^='url(#']");
        expect(area).not.toBeNull();
        expect(area?.getAttribute("d")).toMatch(/Z$/);

        unmount();
    });

    it("draws the commits trace as a curved line above the area", () => {
        const { container, unmount } = mount();

        const line = container.querySelector(
            "path[stroke='var(--foreground)']"
        );
        expect(line?.getAttribute("stroke-width")).toBe("1.5");
        // Monotone interpolation emits cubic segments, not straight lines.
        expect(line?.getAttribute("d")).toMatch(/^M[\d.,-]+C/);

        unmount();
    });

    it("animates only the bars, leaving the commits trace to the CSS wipe", () => {
        const { container, unmount } = mount();

        const roles = new Set(
            [...container.querySelectorAll("[data-ts-motion-role]")].map(
                (node) => node.getAttribute("data-ts-motion-role")
            )
        );
        // The area and line are `motion: false`: the renderer grows paths from
        // the value axis, which is the down-up reveal the CSS wipe replaces.
        expect(roles).toEqual(new Set(["bar"]));

        unmount();
    });

    it("paints the commits trace behind the bars", () => {
        const { container, unmount } = mount();

        // Mark states wrap each series in its own <g>, so read document order
        // across the whole marks layer rather than its direct children.
        const order = [
            ...container.querySelectorAll(
                "svg.ts-chart g.ts-chart__area, svg.ts-chart g.ts-chart__line, svg.ts-chart g.ts-chart__bar"
            ),
        ].map((node) =>
            node.classList.contains("ts-chart__area")
                ? "area"
                : node.classList.contains("ts-chart__line")
                  ? "line"
                  : "bar"
        );

        expect(order).toEqual(["area", "line", "bar", "bar"]);

        unmount();
    });

    it("fades the commits fill downward from the top", () => {
        const { container, unmount } = mount();

        const gradient = container.querySelector("linearGradient");
        expect(gradient?.getAttribute("y1")).toBe("0%");
        expect(gradient?.getAttribute("y2")).toBe("100%");
        expect(
            [...container.querySelectorAll("linearGradient stop")].map((stop) =>
                stop.getAttribute("stop-opacity")
            )
        ).toEqual(["0.32", "0"]);

        unmount();
    });

    it("shares one left-to-right reveal clip with the area and the line", () => {
        const { container, unmount } = mount();

        const clip = container.querySelector("clipPath");
        expect(clip?.getAttribute("clipPathUnits")).toBe("objectBoundingBox");
        const wipe = clip?.querySelector("rect");
        expect(wipe?.getAttribute("class")).toBe("history-chart__reveal");

        const wrapper = container.firstElementChild as HTMLElement;
        expect(wrapper.style.getPropertyValue("--history-chart-reveal")).toBe(
            `url(#${clip?.getAttribute("id")})`
        );
        expect(
            container.querySelector(".ts-chart-host.history-chart")
        ).not.toBeNull();

        unmount();
    });

    it("starts the entrance with collapsed geometry", () => {
        const { container, unmount } = mount();

        for (const rect of marks(container, "var(--success)")) {
            expect(geometry(rect).height).toBe(0);
        }
        expect(
            container.querySelector("svg.ts-chart g.ts-chart__area")
        ).not.toBeNull();
        expect(
            container
                .querySelector("svg.ts-chart g.ts-chart__area")
                ?.getAttribute("transform")
        ).toBeNull();

        unmount();
    });

    it("never grows a bar past its settled height", async () => {
        const { container, settle, unmount } = mount();

        const peak = new Map<string, number>();
        const heightOf = (fill: string): [string, number][] =>
            marks(container, fill).map((rect) => [
                rect.getAttribute("data-ts-key") ?? "",
                geometry(rect).height,
            ]);

        for (let frame = 0; frame < 12; frame++) {
            await act(async () => {
                await new Promise((resolve) => setTimeout(resolve, 120));
            });
            for (const fill of ["var(--success)", "var(--destructive)"]) {
                for (const [key, height] of heightOf(fill)) {
                    expect(height).toBeGreaterThanOrEqual(0);
                    peak.set(key, Math.max(peak.get(key) ?? 0, height));
                }
            }
        }

        await settle();
        for (const fill of ["var(--success)", "var(--destructive)"]) {
            for (const [key, height] of heightOf(fill)) {
                expect(peak.get(key) ?? 0).toBeLessThanOrEqual(height + 0.5);
            }
        }

        unmount();
    });

    it("labels the surface for assistive tech", () => {
        const { container, unmount } = mount();

        expect(
            container.querySelector("svg.ts-chart")?.getAttribute("aria-label")
        ).toBe("Commits and line changes per time bucket");

        unmount();
    });
});
