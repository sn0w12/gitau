import { motion, useReducedMotion } from "motion/react";
import type { Transition } from "motion/react";
import type { ComponentPropsWithoutRef, ElementType, ReactNode } from "react";
import {
    createElement,
    useCallback,
    useLayoutEffect,
    useMemo,
    useRef,
    useState,
} from "react";

import { EASE_SNAPPY } from "@/lib/motion";
import { cn } from "@/lib/utils";

const DEFAULT_TRANSITION: Transition = { duration: 0.2, ease: EASE_SNAPPY };

type AnimatedSizeProps<T extends ElementType = "div"> = {
    as?: T;
    /** Element for the measurement wrapper; a phrasing tag for an inline `as`. */
    innerAs?: ElementType;
    children?: ReactNode;
    /** Animate the intrinsic width. Disable when the parent owns width. */
    width?: boolean;
    /** Animate the intrinsic height. Disable when the parent owns height. */
    height?: boolean;
    transition?: Transition;
    className?: string;
    innerClassName?: string;
} & Omit<
    ComponentPropsWithoutRef<T>,
    "children" | "className" | "width" | "height"
>;

/**
 * Sizes a box to its content and animates changes on the enabled axes.
 *
 * The measurement happens on an inner box that nothing clamps: a `max-w-full`
 * inner would report the frame's current size back and every change would
 * stall. `overflow-hidden` covers content larger than the frame while the
 * animation runs.
 *
 * The inner is observed for resizes and DOM changes rather than measured on
 * renders alone: a parent can skip rendering this component (memoized element,
 * identical props) while a descendant paints content that widens it.
 */
export function AnimatedSize<T extends ElementType = "div">({
    as,
    innerAs = "div",
    children,
    width = true,
    height = true,
    transition = DEFAULT_TRANSITION,
    className,
    innerClassName,
    ...props
}: AnimatedSizeProps<T>) {
    const innerRef = useRef<HTMLElement | null>(null);
    const resizeObserver = useRef<ResizeObserver | null>(null);
    const mutationObserver = useRef<MutationObserver | null>(null);
    const [size, setSize] = useState<{
        width: number | null;
        height: number | null;
    }>({ width: null, height: null });
    const reducedMotion = useReducedMotion();
    const outer = as ?? "div";
    const MotionTag = useMemo(
        () => motion.create(outer) as ElementType,
        [outer]
    );

    const measure = useCallback(() => {
        const inner = innerRef.current;
        if (!inner) return;
        const rect = inner.getBoundingClientRect();
        const measuredWidth = width ? Math.round(rect.width) : null;
        const measuredHeight = height ? Math.round(rect.height) : null;
        setSize((prev) => {
            // Zero means jsdom or a hidden ancestor; keep the last real size
            // instead of collapsing the frame.
            const nextWidth = measuredWidth === 0 ? prev.width : measuredWidth;
            const nextHeight =
                measuredHeight === 0 ? prev.height : measuredHeight;
            if (nextWidth === prev.width && nextHeight === prev.height) {
                return prev;
            }
            return { width: nextWidth, height: nextHeight };
        });
    }, [height, width]);

    const setInner = useCallback(
        (node: HTMLElement | null) => {
            resizeObserver.current?.disconnect();
            mutationObserver.current?.disconnect();
            resizeObserver.current = null;
            mutationObserver.current = null;
            innerRef.current = node;
            if (!node) return;
            if (typeof ResizeObserver !== "undefined") {
                const observer = new ResizeObserver(measure);
                observer.observe(node);
                resizeObserver.current = observer;
            }
            if (typeof MutationObserver !== "undefined") {
                const observer = new MutationObserver(measure);
                observer.observe(node, {
                    childList: true,
                    characterData: true,
                    subtree: true,
                });
                mutationObserver.current = observer;
            }
        },
        [measure]
    );

    useLayoutEffect(measure);

    const animate = {
        ...(width ? { width: size.width ?? "auto" } : {}),
        ...(height ? { height: size.height ?? "auto" } : {}),
    };
    const motionProps = {
        ...props,
        className: cn("overflow-hidden", className),
        animate,
        transition: reducedMotion ? { duration: 0 } : transition,
    };
    const innerProps = {
        ref: setInner,
        className: cn("block", width ? "w-max" : "w-full", innerClassName),
    };

    return (
        <MotionTag {...motionProps}>
            {createElement(innerAs, innerProps, children)}
        </MotionTag>
    );
}
