import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { PointerEvent as ReactPointerEvent } from "react";

import { detectPlatform } from "./platform";

export interface DragRegionProps {
    "data-tauri-drag-region"?: true;
    onPointerDown?: (event: ReactPointerEvent<HTMLDivElement>) => void;
}

// Same clickable set tauri's drag.js uses to decide a press belongs to a
// control rather than to the window.
const CLICKABLE_TAGS = new Set([
    "A",
    "BUTTON",
    "INPUT",
    "SELECT",
    "TEXTAREA",
    "LABEL",
    "SUMMARY",
]);
const INTERACTIVE_ROLES = new Set([
    "button",
    "link",
    "menuitem",
    "tab",
    "checkbox",
    "radio",
    "switch",
    "option",
]);

function isClickable(element: HTMLElement): boolean {
    return (
        CLICKABLE_TAGS.has(element.tagName) ||
        (element.hasAttribute("contenteditable") &&
            element.getAttribute("contenteditable") !== "false") ||
        (element.hasAttribute("tabindex") &&
            element.getAttribute("tabindex") !== "-1") ||
        INTERACTIVE_ROLES.has(element.getAttribute("role") ?? "")
    );
}

function belongsToControl(event: ReactPointerEvent<HTMLDivElement>): boolean {
    for (const node of event.nativeEvent.composedPath()) {
        if (!(node instanceof HTMLElement)) continue;
        if (node === event.currentTarget) return false;
        if (isClickable(node)) return true;
    }
    return false;
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
    // Unlike the native bare region, the whole region drags so the empty
    // titlebar area works even where child elements cover the region element.
    if (belongsToControl(event)) return;
    event.preventDefault();
    // The same press also reaches the outer titlebar region; only this one moves.
    event.stopPropagation();

    const region = event.currentTarget;
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
