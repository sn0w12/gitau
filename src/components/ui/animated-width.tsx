import { motion, useReducedMotion } from "motion/react";
import type { ReactNode } from "react";
import { useCallback, useLayoutEffect, useRef, useState } from "react";

import { EASE_SNAPPY } from "@/lib/motion";

const WIDTH_TRANSITION = { duration: 0.2, ease: EASE_SNAPPY } as const;

/**
 * Sizes a box to the intrinsic width of its content and animates every change,
 * so a value that grows or shrinks resizes in place instead of snapping the
 * surrounding layout sideways.
 *
 * The width is measured from an inner box that nothing clamps: a `max-w-full`
 * inner would report the frame's current width back and every change would
 * stall. `overflow-hidden` covers the content wider than the frame while the
 * animation runs.
 */
export function AnimatedWidth({ children }: { children: ReactNode }) {
    const innerRef = useRef<HTMLSpanElement>(null);
    const [width, setWidth] = useState<number | null>(null);
    const reducedMotion = useReducedMotion();

    const measure = useCallback(() => {
        const inner = innerRef.current;
        if (!inner) return;
        const intrinsic = Math.round(inner.getBoundingClientRect().width);
        // A zero width (jsdom, a hidden settings tab) keeps the last one
        // rather than collapsing the frame.
        setWidth((prev) => (intrinsic === 0 ? prev : intrinsic));
    }, []);

    useLayoutEffect(measure);

    useLayoutEffect(() => {
        const inner = innerRef.current;
        if (!inner || typeof ResizeObserver === "undefined") return;
        const observer = new ResizeObserver(measure);
        observer.observe(inner);
        return () => observer.disconnect();
    }, [measure]);

    return (
        <motion.span
            className="inline-flex overflow-hidden"
            animate={{ width: width ?? "auto" }}
            transition={reducedMotion ? { duration: 0 } : WIDTH_TRANSITION}
        >
            <span ref={innerRef} className="block w-max">
                {children}
            </span>
        </motion.span>
    );
}
