import { ChevronsDown, ChevronsUp } from "lucide-react";
import type { ComponentType } from "react";

import { Spinner } from "@/components/ui/spinner";
import { TooltipCreateHandle, TooltipTrigger } from "@/components/ui/tooltip";
import type {
    ControlSpec,
    ExpandWindow,
    GapControls,
} from "@/lib/diff/context-gaps";
import { cn } from "@/lib/utils";

export type DiffTooltipHandle = ReturnType<
    typeof TooltipCreateHandle<ComponentType>
>;

/**
 * Replaces the two line-number gutters and the marker column of a hunk header,
 * reusing their exact cell widths so each button's right edge lands on the same
 * offset as the line numbers above and below it.
 */
export function HunkHeaderControls({
    controls,
    tooltipHandle,
    onExpand,
}: {
    controls: GapControls;
    tooltipHandle: DiffTooltipHandle;
    onExpand: (
        gapKey: number,
        direction: "up" | "down",
        window: ExpandWindow
    ) => void;
}) {
    return (
        <>
            <span className="flex w-10 shrink-0 items-center justify-end pr-1.25">
                <ExpandButton
                    spec={controls.up}
                    tooltipHandle={tooltipHandle}
                    onExpand={onExpand}
                />
            </span>
            <span className="flex w-10 shrink-0 items-center justify-end pr-1.25">
                <ExpandButton
                    spec={controls.down}
                    tooltipHandle={tooltipHandle}
                    onExpand={onExpand}
                />
            </span>
            <span className="w-4 shrink-0" />
        </>
    );
}

function ExpandButton({
    spec,
    tooltipHandle,
    onExpand,
}: {
    spec: ControlSpec | null;
    tooltipHandle: DiffTooltipHandle;
    onExpand: (
        gapKey: number,
        direction: "up" | "down",
        window: ExpandWindow
    ) => void;
}) {
    if (!spec) return null;
    const up = spec.direction === "up";
    const label = `Expand ${spec.window.count} lines ${up ? "above" : "below"}`;
    const payload = () => <span>{label}</span>;
    return (
        <TooltipTrigger
            handle={tooltipHandle}
            payload={payload}
            render={
                <button
                    type="button"
                    aria-label={label}
                    disabled={spec.pending}
                    onClick={() =>
                        onExpand(spec.gapKey, spec.direction, spec.window)
                    }
                    className={cn(
                        "flex h-4 items-center gap-0.5 rounded-sm pr-0.75 text-xs",
                        "text-muted-foreground",
                        "hover:bg-muted hover:text-foreground disabled:opacity-60"
                    )}
                >
                    {spec.pending ? (
                        <Spinner className="size-3" />
                    ) : up ? (
                        <ChevronsUp className="size-3" />
                    ) : (
                        <ChevronsDown className="size-3" />
                    )}
                    <span className="tabular-nums">{spec.window.count}</span>
                </button>
            }
        />
    );
}
