import { RefreshCw } from "lucide-react";
import {
    motion,
    useAnimationFrame,
    useMotionValue,
    useSpring,
    type MotionValue,
} from "motion/react";
import type { ReactNode } from "react";
import { useEffect, useRef, useState } from "react";

import { cn } from "@/lib/utils";

// Degrees per second the spinner settles into. The spring below chases a target
// advancing at this rate, so the icon winds up from rest on start and coasts to a
// stop on finish instead of snapping between static and full speed.
const SPIN_SPEED_DEG_PER_SEC = 360;
const SPIN_SPRING = { stiffness: 120, damping: 16, mass: 1 } as const;
const ICON_SLOT_CLASS = "absolute inset-0 flex items-center justify-center";
// Motion writes the rotation as an inline style every frame, which Chromium
// does not promote the way it does an `animate-spin` keyframe: it re-rasterizes
// the SVG at each new angle, and on Windows the GPU rasterizer leaves stray
// pixels along the stroke edges. `will-change` keeps one raster and rotates it.
const SPIN_LAYER_CLASS = "will-change-transform";
const ICON_SWAP_TRANSITION = { duration: 0.18, ease: "easeOut" } as const;
const BLURRED = "blur(4px)";
const SHARP = "blur(0px)";

type SpinPhase = "idle" | "spinning" | "settling";

/**
 * Fetch has no separate idle icon: the same `RefreshCw` stays mounted and only
 * its rotation is driven. Pull and push pass an `idleIcon`; the two glyphs
 * cross-fade through opacity and blur, and the handoff back waits for the spin
 * to finish its turn (`settling` -> `idle`) so the icon never swaps mid-spin.
 */
export function SpinnerIcon({
    busy,
    idleIcon,
}: {
    busy: boolean;
    idleIcon?: ReactNode;
}) {
    const target = useMotionValue(0);
    const rotate = useSpring(target, SPIN_SPRING);
    const [phase, setPhase] = useState<SpinPhase>(busy ? "spinning" : "idle");
    const [wasBusy, setWasBusy] = useState(busy);
    const hasCustomIdle = idleIcon !== undefined;
    const showSpinner = !hasCustomIdle || phase !== "idle";

    // Track the busy edge during render rather than in an effect, so the phase
    // follows the prop without a second render pass.
    if (busy !== wasBusy) {
        setWasBusy(busy);
        setPhase((current) => {
            if (busy) return "spinning";
            return current === "spinning" ? "settling" : current;
        });
    }

    // Finish the turn the spin is partway through so the swap back to the idle
    // glyph lands on a completed rotation. `ceil` only moves forward, so it can
    // never rewind the way a nearest-multiple snap can.
    useEffect(() => {
        if (phase !== "settling") return;
        const angle = target.get();
        const nextTurn = Math.ceil(angle / 360) * 360;
        if (nextTurn !== angle) target.set(nextTurn);
    }, [phase, target]);

    return (
        <span className="relative size-7 shrink-0">
            {phase !== "idle" && (
                <SpinDriver
                    target={target}
                    rotate={rotate}
                    spinning={phase === "spinning"}
                    onSettled={() => setPhase("idle")}
                />
            )}
            <motion.span
                aria-hidden={hasCustomIdle && !showSpinner ? true : undefined}
                animate={
                    hasCustomIdle
                        ? {
                              opacity: showSpinner ? 1 : 0,
                              filter: showSpinner ? SHARP : BLURRED,
                          }
                        : undefined
                }
                className={cn(
                    ICON_SLOT_CLASS,
                    phase !== "idle" && SPIN_LAYER_CLASS
                )}
                style={{ rotate }}
                transition={ICON_SWAP_TRANSITION}
            >
                <RefreshCw
                    className="size-7"
                    strokeWidth="1.5px"
                    data-testid={busy ? "fetch-spinner" : undefined}
                />
            </motion.span>
            {hasCustomIdle && (
                <motion.span
                    aria-hidden={showSpinner ? true : undefined}
                    animate={{
                        opacity: showSpinner ? 0 : 1,
                        filter: showSpinner ? BLURRED : SHARP,
                    }}
                    className={ICON_SLOT_CLASS}
                    transition={ICON_SWAP_TRANSITION}
                >
                    {idleIcon}
                </motion.span>
            )}
        </span>
    );
}

/**
 * Advances the spin target while spinning, then watches the spring until it
 * settles. Mounted only while the spin is active, so the animation frame that
 * keeps rescheduling itself is gone once the icon is idle.
 */
function SpinDriver({
    target,
    rotate,
    spinning,
    onSettled,
}: {
    target: MotionValue<number>;
    rotate: MotionValue<number>;
    spinning: boolean;
    onSettled: () => void;
}) {
    const settledRef = useRef(false);

    useAnimationFrame((_, delta) => {
        if (spinning) {
            settledRef.current = false;
            target.set(target.get() + (SPIN_SPEED_DEG_PER_SEC * delta) / 1000);
            return;
        }

        if (settledRef.current) return;
        const close = Math.abs(rotate.get() - target.get()) < 0.5;
        const slow = Math.abs(rotate.getVelocity()) < 8;
        if (close && slow) {
            settledRef.current = true;
            onSettled();
        }
    });

    return null;
}
