import type { Align } from "@base-ui/react/internals/useAnchorPositioning";
import { motion, type Transition } from "motion/react";
import type { ReactNode } from "react";

import { EASE_SNAPPY } from "@/lib/motion";
import { cn } from "@/lib/utils";

const DEFAULT_TRANSITION: Transition = { duration: 0.16, ease: EASE_SNAPPY };

interface SwapProps {
    /** When true, `on` is shown; otherwise `off`. */
    when: boolean;
    /** Content rendered when `when === true`. */
    on: ReactNode;
    /** Content rendered when `when === false`. */
    off: ReactNode;
    /** Classes for the stacking wrapper (sizing, etc.). */
    className?: string;
    /** Motion transition override. */
    transition?: Transition;
    /**
     * Blur applied to the outgoing element while it fades. Pass a CSS length
     * (e.g. `"3px"`, `"6px"`) to tune it, or `false` to disable the blur and
     * cross-fade only.
     */
    blur?: string | false;
    align?: Align;
}

const alignClass: Record<Align, string> = {
    start: "place-items-start",
    center: "place-items-center",
    end: "place-items-end",
};

/**
 * Renders two React elements stacked in the same grid cell and cross-fades
 * between them. The wrapper sizes to the larger of the two, so swapping never
 * causes a layout jump, and the outgoing element gets a subtle blur so a
 * shape change reads as one element morphing instead of two swapping.
 */
export function Swap({
    when,
    on,
    off,
    className,
    transition = DEFAULT_TRANSITION,
    blur = "3px",
    align = "center",
}: SwapProps) {
    const blurred = blur === false ? "blur(0px)" : `blur(${blur})`;
    const sharp = "blur(0px)";

    return (
        <span className={cn("relative grid", alignClass[align], className)}>
            <motion.span
                className="col-start-1 row-start-1"
                aria-hidden={when}
                style={{ pointerEvents: when ? "none" : undefined }}
                initial={false}
                animate={{
                    opacity: when ? 0 : 1,
                    filter: when ? blurred : sharp,
                }}
                transition={transition}
            >
                {off}
            </motion.span>
            <motion.span
                className="col-start-1 row-start-1"
                aria-hidden={!when}
                style={{ pointerEvents: when ? undefined : "none" }}
                initial={false}
                animate={{
                    opacity: when ? 1 : 0,
                    filter: when ? sharp : blurred,
                }}
                transition={transition}
            >
                {on}
            </motion.span>
        </span>
    );
}
