import * as React from "react";

/**
 * Fixed-height windowing over a scroll container: rows never wrap, so plain
 * index arithmetic replaces a measurement library. Geometry is published
 * from a ResizeObserver (its initial notification fires on observe) and a
 * passive scroll listener, never read during render; zero-height elements
 * (jsdom, hidden tabs) stay on the deterministic fallback viewport.
 */
export function useFixedHeightWindow({
    count,
    rowHeight,
    containerRef,
    overscan = 10,
    fallbackViewportHeight = 600,
}: {
    count: number;
    rowHeight: number;
    containerRef: React.RefObject<HTMLDivElement | null>;
    overscan?: number;
    fallbackViewportHeight?: number;
}): {
    start: number;
    end: number;
    totalHeight: number;
    offsetTop: number;
} {
    const [geometry, setGeometry] = React.useState({
        scrollTop: 0,
        viewportHeight: fallbackViewportHeight,
    });

    React.useLayoutEffect(() => {
        const el = containerRef.current;
        if (!el) return;

        const captureScroll = () => {
            setGeometry((prev) =>
                prev.scrollTop === el.scrollTop
                    ? prev
                    : { ...prev, scrollTop: el.scrollTop }
            );
        };
        el.addEventListener("scroll", captureScroll, { passive: true });

        if (typeof ResizeObserver === "undefined") {
            const raf = requestAnimationFrame(() => {
                const rect = el.getBoundingClientRect();
                setGeometry((prev) => {
                    const viewportHeight =
                        rect.height > 0 ? rect.height : fallbackViewportHeight;
                    return prev.viewportHeight === viewportHeight &&
                        prev.scrollTop === el.scrollTop
                        ? prev
                        : { scrollTop: el.scrollTop, viewportHeight };
                });
            });
            return () => {
                cancelAnimationFrame(raf);
                el.removeEventListener("scroll", captureScroll);
            };
        }

        const observer = new ResizeObserver(() => {
            const rect = el.getBoundingClientRect();
            setGeometry((prev) => {
                const viewportHeight =
                    rect.height > 0 ? rect.height : fallbackViewportHeight;
                return prev.viewportHeight === viewportHeight &&
                    prev.scrollTop === el.scrollTop
                    ? prev
                    : { scrollTop: el.scrollTop, viewportHeight };
            });
        });
        observer.observe(el);
        return () => {
            observer.disconnect();
            el.removeEventListener("scroll", captureScroll);
        };
    }, [containerRef, fallbackViewportHeight]);

    const firstVisible = Math.floor(geometry.scrollTop / rowHeight);
    const visibleCount = Math.ceil(geometry.viewportHeight / rowHeight);

    const start = Math.max(0, firstVisible - overscan);
    const end = Math.min(count, firstVisible + visibleCount + overscan);
    const totalHeight = count * rowHeight;

    return {
        start,
        end,
        totalHeight,
        offsetTop: start * rowHeight,
    };
}
