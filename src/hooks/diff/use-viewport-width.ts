import * as React from "react";

/**
 * Tracks the scroll viewport's clientWidth so wrapped diff rows can budget
 * characters per line. Zero-width viewports (jsdom, hidden tabs) stay 0 and
 * callers fall back to an unwrapped layout.
 */
export function useViewportWidth(
    ref: React.RefObject<HTMLDivElement | null>
): number {
    const [width, setWidth] = React.useState(0);

    React.useLayoutEffect(() => {
        const el = ref.current;
        if (!el) return;
        const measure = () => {
            setWidth((prev) =>
                prev === el.clientWidth ? prev : el.clientWidth
            );
        };
        measure();
        if (typeof ResizeObserver === "undefined") {
            const raf = requestAnimationFrame(measure);
            return () => {
                cancelAnimationFrame(raf);
            };
        }
        const observer = new ResizeObserver(measure);
        observer.observe(el);
        return () => {
            observer.disconnect();
        };
    }, [ref]);

    return width;
}
