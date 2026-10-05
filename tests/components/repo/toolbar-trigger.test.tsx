import { act } from "react";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";
// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import {
    ToolbarTrigger,
    ToolbarTriggerFrame,
} from "@/components/repo/toolbar-trigger";

(
    globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

// jsdom has no layout, so the intrinsic width the frame measures is stubbed.
let intrinsicWidth = 0;

Object.defineProperty(Element.prototype, "getBoundingClientRect", {
    configurable: true,
    value(this: Element) {
        return {
            ...DOMRect.fromRect({ width: intrinsicWidth, height: 56 }),
            width: intrinsicWidth,
            x: 0,
            y: 0,
        };
    },
});

// jsdom has no ResizeObserver either. A stand-in whose callbacks the test
// drives covers the path where the content resizes without a re-render, which
// is how Tailwind's `lg:` breakpoints and font swaps reach the frame.
const observers = new Set<() => void>();

class StubResizeObserver {
    constructor(private readonly callback: () => void) {}

    observe(): void {
        observers.add(this.callback);
    }

    disconnect(): void {
        observers.delete(this.callback);
    }
}

(globalThis as unknown as { ResizeObserver: unknown }).ResizeObserver =
    StubResizeObserver;

function notifyObservers(): void {
    act(() => {
        for (const observer of observers) observer();
    });
}

beforeEach(() => {
    intrinsicWidth = 0;
    observers.clear();
});

afterEach(() => {
    observers.clear();
});

function Host({ label }: { label: string }): ReactElement {
    return (
        <ToolbarTriggerFrame>
            <button type="button">{label}</button>
        </ToolbarTriggerFrame>
    );
}

function render(ui: ReactElement) {
    const container = document.createElement("div");
    document.body.appendChild(container);
    const root = createRoot(container);
    act(() => {
        root.render(ui);
    });
    return {
        container,
        frame: () => container.firstElementChild as HTMLElement,
        width: () => (container.firstElementChild as HTMLElement).style.width,
        rerender(next: ReactElement) {
            act(() => {
                root.render(next);
            });
        },
        unmount() {
            act(() => root.unmount());
            container.remove();
        },
    };
}

/** Waits out the width animation rather than guessing how long it takes. */
async function widthSettlesAt(
    read: () => string,
    expected: string
): Promise<void> {
    const deadline = Date.now() + 2_000;
    while (Date.now() < deadline) {
        if (read() === expected) return;
        await act(async () => {
            await new Promise((resolve) => setTimeout(resolve, 25));
        });
    }
    expect(read()).toBe(expected);
}

async function tick(count: number): Promise<void> {
    for (let i = 0; i < count; i++) {
        await act(async () => {
            await new Promise((resolve) => setTimeout(resolve, 25));
        });
    }
}

describe("ToolbarTriggerFrame", () => {
    it("keeps the frame from shrinking, so growth is never absorbed", () => {
        intrinsicWidth = 256;
        const view = render(<Host label="Fetch Origin" />);

        const frame = view.frame();
        // A shrinkable frame lets flexbox hand every increase back to the row,
        // which pins the rendered width at the floor while the target moves.
        expect(frame.className).toContain("shrink-0");
        view.unmount();
    });

    it("animates to the intrinsic width of its content", async () => {
        intrinsicWidth = 320;
        const view = render(<Host label="Fetch Origin" />);
        await widthSettlesAt(view.width, "320px");
        view.unmount();
    });

    it("animates again when the content width changes", async () => {
        intrinsicWidth = 240;
        const view = render(<Host label="Fetch Origin" />);
        await widthSettlesAt(view.width, "240px");

        intrinsicWidth = 288;
        view.rerender(<Host label="Push Origin" />);
        await widthSettlesAt(view.width, "288px");
        view.unmount();
    });

    it("follows a resize the observer reports without a re-render", async () => {
        intrinsicWidth = 256;
        const view = render(<Host label="Push Origin" />);
        await widthSettlesAt(view.width, "256px");

        intrinsicWidth = 210;
        notifyObservers();
        await widthSettlesAt(view.width, "210px");
        view.unmount();
    });

    it("keeps the last width while the content measures zero", async () => {
        intrinsicWidth = 256;
        const view = render(<Host label="Push Origin" />);
        await widthSettlesAt(view.width, "256px");

        intrinsicWidth = 0;
        view.rerender(<Host label="Push Origin" />);
        notifyObservers();
        await tick(8);

        expect(view.width()).toBe("256px");
        view.unmount();
    });
});

describe("ToolbarTrigger", () => {
    it("keeps the trigger inside the measured frame", () => {
        intrinsicWidth = 200;
        const view = render(
            <ToolbarTrigger data-testid="fetch-button">
                Push Origin
            </ToolbarTrigger>
        );

        const inner = view.frame().firstElementChild as HTMLElement;
        expect(inner.className).toContain("w-max");
        // Nothing may clamp the measuring box: a clamp makes the measurement
        // report the frame's current width and every change stalls.
        expect(inner.className).not.toContain("max-w-");
        const trigger = inner.querySelector(
            '[data-testid="fetch-button"]'
        ) as HTMLElement;
        // A button shrink-wraps to its content even at display:flex, which
        // leaves justify-between no free space for the trailing counts.
        expect(trigger.className).toContain("w-full");
        expect(trigger.className).not.toContain("w-auto");
        view.unmount();
    });
});
