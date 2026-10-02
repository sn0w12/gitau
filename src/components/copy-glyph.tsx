import { Check, Copy } from "lucide-react";
import { motion } from "motion/react";

import { EASE_SNAPPY } from "@/lib/motion";

const SWAP_TRANSITION = { duration: 0.16, ease: EASE_SNAPPY } as const;
const BLURRED = "blur(3px)";
const SHARP = "blur(0px)";

/** Copy icon that cross-fades to a checkmark, so a shape change reads as one
 *  icon morphing rather than two glyphs swapping. */
export function CopyGlyph({ copied }: { copied: boolean }) {
    return (
        <span className="relative grid place-items-center">
            <motion.span
                className="col-start-1 row-start-1"
                animate={{
                    opacity: copied ? 0 : 1,
                    filter: copied ? BLURRED : SHARP,
                }}
                transition={SWAP_TRANSITION}
            >
                <Copy />
            </motion.span>
            <motion.span
                className="col-start-1 row-start-1"
                animate={{
                    opacity: copied ? 1 : 0,
                    filter: copied ? SHARP : BLURRED,
                }}
                transition={SWAP_TRANSITION}
            >
                <Check />
            </motion.span>
        </span>
    );
}
