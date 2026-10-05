import { motion, useReducedMotion } from "motion/react";
import type { ComponentProps, ReactNode } from "react";
import { useCallback, useLayoutEffect, useRef, useState } from "react";

import { BORDER_GRADIENT, REPO_TOOLBAR_TRIGGER_CLASS } from "@/lib/constants";
import { EASE_SNAPPY } from "@/lib/motion";
import { cn } from "@/lib/utils";

const WIDTH_TRANSITION = { duration: 0.2, ease: EASE_SNAPPY } as const;

const ACTION_TRIGGER_EXTRAS =
    "text-left transition-opacity duration-200 disabled:pointer-events-none disabled:opacity-64";

// The floor belongs on the inner because that is the box the trigger fills. The
// frame must not shrink: three columns floored at the same width already fill
// most panels, and a shrinkable frame lets flexbox absorb every increase, so a
// trigger that needs more room would silently stay the same width.
const COLUMN_MIN = "min-w-32 lg:min-w-64";

// The frame carries the chrome because its width is the one that moves: the
// border, the hover fill and the divider have to track a width in flight. It
// also carries the focus ring, which `overflow-hidden` would otherwise clip off
// the trigger inside it.
const FRAME_CLASS = cn(
    "h-full shrink-0 cursor-pointer overflow-hidden border-b hover:bg-accent has-[:focus-visible]:ring-2 has-[:focus-visible]:ring-ring has-[:focus-visible]:ring-offset-1 has-[:focus-visible]:ring-offset-background",
    BORDER_GRADIENT
);

// `h-full` on the inner is what gives the trigger a definite height to fill.
// Without it `h-full` on the trigger falls back to its content height, which
// parks the trigger at the top of the frame and shifts the content a few
// pixels high whenever the subtitle is hidden.
const MEASURE_CLASS = cn("block h-full w-max", COLUMN_MIN);

/**
 * The row's triggers size themselves to what they hold, so a label swap, a
 * subtitle that grew, or a counts badge appearing resized them and shoved the
 * rest of the row sideways in one frame. This animates that resize instead.
 *
 * The width is measured from an inner that nothing clamps, which is what makes
 * the measurement trustworthy: a `max-w-full` inner would report the frame's
 * current width back and every change would stall. `overflow-hidden` on the
 * frame covers the two moments when the content is wider than the frame, while
 * the animation runs and while a narrow panel squeezes the row.
 */
export function ToolbarTriggerFrame({ children }: { children: ReactNode }) {
    const innerRef = useRef<HTMLSpanElement>(null);
    const [width, setWidth] = useState<number | null>(null);
    const reducedMotion = useReducedMotion();

    const measure = useCallback(() => {
        const inner = innerRef.current;
        if (!inner) return;
        // A zero width (jsdom, a hidden tab) keeps the last one rather than
        // collapsing the frame.
        const intrinsic = Math.round(inner.getBoundingClientRect().width);
        setWidth((prev) => (intrinsic === 0 ? prev : intrinsic));
    }, []);

    useLayoutEffect(measure);

    // The `lg:` breakpoints and font swaps resize the content without a render,
    // so only an observer sees them.
    useLayoutEffect(() => {
        const inner = innerRef.current;
        if (!inner || typeof ResizeObserver === "undefined") return;
        const observer = new ResizeObserver(measure);
        observer.observe(inner);
        return () => observer.disconnect();
    }, [measure]);

    return (
        <motion.div
            className={FRAME_CLASS}
            animate={{ width: width ?? "auto" }}
            transition={reducedMotion ? { duration: 0 } : WIDTH_TRANSITION}
        >
            <span ref={innerRef} className={MEASURE_CLASS}>
                {children}
            </span>
        </motion.div>
    );
}

/** Toolbar trigger for the buttons that act on the repo rather than open a menu. */
export function ToolbarTrigger({
    className,
    ...props
}: ComponentProps<"button">) {
    return (
        <ToolbarTriggerFrame>
            <button
                className={cn(
                    REPO_TOOLBAR_TRIGGER_CLASS,
                    ACTION_TRIGGER_EXTRAS,
                    className
                )}
                {...props}
            />
        </ToolbarTriggerFrame>
    );
}
