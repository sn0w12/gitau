import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { PointerEvent as ReactPointerEvent } from "react";

import { detectPlatform } from "./platform";

export interface DragRegionProps {
    "data-tauri-drag-region"?: true;
    onPointerDown?: (event: ReactPointerEvent<HTMLDivElement>) => void;
}

// Window drag on Linux/X11: Chromium's X connection owns the pointer grab, so
// the window manager ignores the _NET_WM_MOVERESIZE that startDragging() (and
// the data-tauri-drag-region handler) asks for, and a frameless window simply
// never moves. It does not crash. See the X11 note in the CEF migration thread:
// https://github.com/tauri-apps/tauri/issues/14963#issuecomment-5965777662
// Move the window from screen-space pointer deltas instead, and drop this once
// upstream handles drag on Linux CEF.
function dragFromPointer(event: ReactPointerEvent<HTMLDivElement>) {
    if (event.button !== 0 || !event.isPrimary) return;

    const region = event.currentTarget;
    // Tabs and window controls are children with their own pointer handling;
    // only a press on the region itself starts a move.
    if (event.target !== region) return;
    event.preventDefault();

    const appWindow = getCurrentWindow();
    const pointerX = event.screenX;
    const pointerY = event.screenY;
    let origin: PhysicalPosition | null = null;
    let scale = 1;

    const onMove = (move: PointerEvent) => {
        if (!origin) return;
        void appWindow.setPosition(
            new PhysicalPosition(
                Math.round(origin.x + (move.screenX - pointerX) * scale),
                Math.round(origin.y + (move.screenY - pointerY) * scale)
            )
        );
    };
    const stop = () => {
        region.removeEventListener("pointermove", onMove);
        region.removeEventListener("pointerup", stop);
        region.removeEventListener("pointercancel", stop);
    };

    region.setPointerCapture(event.pointerId);
    region.addEventListener("pointermove", onMove);
    region.addEventListener("pointerup", stop);
    region.addEventListener("pointercancel", stop);

    void (async () => {
        // A maximized window cannot be moved, and dragging one is expected to
        // restore it, which unmaximize does before the deltas start applying.
        if (await appWindow.isMaximized()) {
            await appWindow.unmaximize();
        }
        const [position, factor] = await Promise.all([
            appWindow.outerPosition(),
            appWindow.scaleFactor(),
        ]);
        origin = position;
        scale = factor;
    })();
}

export function dragRegionProps(): DragRegionProps {
    return detectPlatform() === "gnome"
        ? { onPointerDown: dragFromPointer }
        : { "data-tauri-drag-region": true };
}
