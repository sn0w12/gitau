import * as React from "react";

const FALLBACK_VIEWPORT_HEIGHT = 600;

/**
 * Fixed-height row window for a list that shares one scroll viewport with
 * other content. The range comes from the list container's offset inside
 * the viewport, so a section can sit anywhere in the scroll content and
 * still window from the viewport's scroll position. Zero-height viewports
 * (jsdom, hidden tabs) fall back to a deterministic height.
 */
export function useWindowedRows({
    count,
    rowHeight,
    containerRef,
    overscan = 6,
}: {
    count: number;
    rowHeight: number;
    containerRef: React.RefObject<HTMLElement | null>;
    overscan?: number;
}): { start: number; end: number } {
    const [geometry, setGeometry] = React.useState({
        top: 0,
        viewportHeight: FALLBACK_VIEWPORT_HEIGHT,
    });

    React.useLayoutEffect(() => {
        const container = containerRef.current;
        const viewport = container?.closest<HTMLElement>(
            '[data-slot="scroll-area-viewport"]'
        );
        if (!container || !viewport) return;

        const measure = () => {
            const viewportHeight =
                viewport.clientHeight || FALLBACK_VIEWPORT_HEIGHT;
            const top =
                container.getBoundingClientRect().top -
                viewport.getBoundingClientRect().top;
            setGeometry((prev) =>
                prev.top === top && prev.viewportHeight === viewportHeight
                    ? prev
                    : { top, viewportHeight }
            );
        };

        measure();
        viewport.addEventListener("scroll", measure, { passive: true });
        if (typeof ResizeObserver === "undefined") {
            return () => viewport.removeEventListener("scroll", measure);
        }
        // Content height changes (filtering, collapsing a section) move this
        // list; watching it keeps the window aligned without scroll events.
        const observer = new ResizeObserver(measure);
        observer.observe(viewport);
        const content = viewport.querySelector(
            '[data-slot="scroll-area-content"]'
        );
        if (content) observer.observe(content);
        return () => {
            viewport.removeEventListener("scroll", measure);
            observer.disconnect();
        };
    }, [containerRef]);

    const firstVisible = Math.max(
        0,
        Math.min(count - 1, Math.floor(-geometry.top / rowHeight))
    );
    const visibleCount = Math.ceil(geometry.viewportHeight / rowHeight);
    return {
        start: Math.max(0, firstVisible - overscan),
        end: Math.min(count, firstVisible + visibleCount + overscan),
    };
}
