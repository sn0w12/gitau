import { AnimatePresence, motion } from "motion/react";
import type { ReactNode } from "react";

import { EASE_SNAPPY } from "@/lib/motion";

const SWAP_TRANSITION = { duration: 0.16, ease: EASE_SNAPPY } as const;
const BLURRED = "blur(3px)";
const SHARP = "blur(0px)";

/**
 * Cross-fades between children as `valueKey` changes: the outgoing node blurs
 * out while the incoming one blurs in, so a swap reads as one thing morphing
 * rather than two swapping. Both nodes are stacked in the same grid cell so the
 * overlap happens in place instead of side by side.
 */
export function BlurCrossfade({
    valueKey,
    children,
}: {
    valueKey: string;
    children: ReactNode;
}) {
    return (
        <span className="grid items-center">
            <AnimatePresence initial={false}>
                <motion.span
                    key={valueKey}
                    className="col-start-1 row-start-1"
                    initial={{ opacity: 0, filter: BLURRED }}
                    animate={{ opacity: 1, filter: SHARP }}
                    exit={{ opacity: 0, filter: BLURRED }}
                    transition={SWAP_TRANSITION}
                >
                    {children}
                </motion.span>
            </AnimatePresence>
        </span>
    );
}
