import { PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { PointerEvent as ReactPointerEvent } from "react";
import { useState } from "react";

import { cn } from "@/lib/utils";

import { detectPlatform } from "./platform";

type ResizeDirection = "n" | "s" | "e" | "w" | "ne" | "nw" | "se" | "sw";

interface ResizeFrame {
    x: number;
    y: number;
    width: number;
    height: number;
    scale: number;
}

// Mirrors app > windows > minWidth / minHeight in tauri.conf.json; the window's
// WM size hints do not constrain a programmatic setSize on Linux.
const MIN_WIDTH = 500;
const MIN_HEIGHT = 480;

const HANDLES: { direction: ResizeDirection; className: string }[] = [
    {
        direction: "n",
        className: "top-0 right-2 left-2 h-1.5 cursor-ns-resize",
    },
    {
        direction: "s",
        className: "right-2 bottom-0 left-2 h-1.5 cursor-ns-resize",
    },
    {
        direction: "w",
        className: "top-2 bottom-2 left-0 w-1.5 cursor-ew-resize",
    },
    {
        direction: "e",
        className: "top-2 right-0 bottom-2 w-1.5 cursor-ew-resize",
    },
    { direction: "nw", className: "top-0 left-0 size-3 cursor-nwse-resize" },
    { direction: "ne", className: "top-0 right-0 size-3 cursor-nesw-resize" },
    { direction: "sw", className: "bottom-0 left-0 size-3 cursor-nesw-resize" },
    {
        direction: "se",
        className: "right-0 bottom-0 size-3 cursor-nwse-resize",
    },
];

function startResize(
    direction: ResizeDirection,
    event: ReactPointerEvent<HTMLDivElement>
) {
    if (event.button !== 0 || !event.isPrimary) return;
    event.preventDefault();

    const handle = event.currentTarget;
    const appWindow = getCurrentWindow();
    const pointerX = event.screenX;
    const pointerY = event.screenY;
    let frame: ResizeFrame | null = null;

    const onMove = (move: PointerEvent) => {
        if (!frame) return;
        const dx = (move.screenX - pointerX) * frame.scale;
        const dy = (move.screenY - pointerY) * frame.scale;

        let width = frame.width;
        let height = frame.height;
        if (direction.includes("e")) width += dx;
        if (direction.includes("w")) width -= dx;
        if (direction.includes("s")) height += dy;
        if (direction.includes("n")) height -= dy;
        width = Math.round(Math.max(MIN_WIDTH, width));
        height = Math.round(Math.max(MIN_HEIGHT, height));

        // The opposite edge stays put, so a west or north resize moves the
        // origin by however much the clamped size differs from the requested.
        const x = direction.includes("w")
            ? frame.x + frame.width - width
            : frame.x;
        const y = direction.includes("n")
            ? frame.y + frame.height - height
            : frame.y;

        void appWindow.setSize(new PhysicalSize(width, height));
        if (x !== frame.x || y !== frame.y) {
            void appWindow.setPosition(new PhysicalPosition(x, y));
        }
    };
    const stop = () => {
        handle.removeEventListener("pointermove", onMove);
        handle.removeEventListener("pointerup", stop);
        handle.removeEventListener("pointercancel", stop);
    };

    handle.setPointerCapture(event.pointerId);
    handle.addEventListener("pointermove", onMove);
    handle.addEventListener("pointerup", stop);
    handle.addEventListener("pointercancel", stop);

    void (async () => {
        if (await appWindow.isMaximized()) {
            await appWindow.unmaximize();
        }
        const [position, size, scale] = await Promise.all([
            appWindow.outerPosition(),
            appWindow.outerSize(),
            appWindow.scaleFactor(),
        ]);
        frame = {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
            scale,
        };
    })();
}

// Window resize on Linux/X11: an undecorated window gets no WM resize borders,
// so resize from edge and corner handles instead. Same source as the drag shim:
// https://github.com/tauri-apps/tauri/issues/14963#issuecomment-5965777662
export function WindowResizeHandles() {
    const [enabled] = useState(() => detectPlatform() === "gnome");
    if (!enabled) return null;

    return (
        <>
            {HANDLES.map(({ direction, className }) => (
                <div
                    key={direction}
                    data-resize-direction={direction}
                    aria-hidden="true"
                    className={cn("fixed z-50", className)}
                    onPointerDown={(event) => startResize(direction, event)}
                />
            ))}
        </>
    );
}
