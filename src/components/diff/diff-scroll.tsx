import type * as React from "react";

import { ScrollAreaPrimitive, ScrollBar } from "@/components/ui/scroll-area";

// Pre-sized from every row of the section, including offscreen ones, so
// the horizontal scrollbar and row backgrounds stay stable while streaming;
// vertical virtualization offsets rows with padding-top.
export function DiffScroll({
    viewportRef,
    totalHeight,
    paddingTop,
    contentMinWidthPx = 0,
    children,
}: {
    viewportRef: React.RefObject<HTMLDivElement | null>;
    totalHeight: number;
    paddingTop: number;
    contentMinWidthPx?: number;
    children: React.ReactNode;
}) {
    return (
        <ScrollAreaPrimitive.Root className="size-full min-h-0">
            <ScrollAreaPrimitive.Viewport
                ref={viewportRef}
                className="group flex h-full flex-col outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-background"
            >
                <ScrollAreaPrimitive.Content>
                    <div
                        className="relative min-w-full font-mono text-xs not-group-data-has-overflow-y:border-b"
                        style={{
                            height: totalHeight,
                            paddingTop,
                            width: `max(100%, ${contentMinWidthPx}px)`,
                        }}
                    >
                        {children}
                    </div>
                </ScrollAreaPrimitive.Content>
                <div
                    className="carbon flex-1"
                    style={{ width: `max(100%, ${contentMinWidthPx}px)` }}
                />
            </ScrollAreaPrimitive.Viewport>
            <ScrollBar orientation="vertical" />
            <ScrollBar orientation="horizontal" />
            <ScrollAreaPrimitive.Corner data-slot="scroll-area-corner" />
        </ScrollAreaPrimitive.Root>
    );
}
