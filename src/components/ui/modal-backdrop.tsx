"use client";

import { useLayoutEffect, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";

import { cn } from "@/lib/utils";

let openDialogs = 0;
const listeners = new Set<() => void>();

function emit(): void {
    for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
    listeners.add(listener);
    return () => listeners.delete(listener);
}

function getSnapshot(): number {
    return openDialogs;
}

/**
 * Chrome drops `backdrop-filter` while an element's opacity is below 1, so a
 * backdrop that fades itself also drops its blur. One shared layer below the
 * dialogs owns the blur and the dim: it stays put while one dialog replaces
 * another, and it interpolates the blur on open and close.
 */
export function useModalBackdrop(open: boolean): void {
    useLayoutEffect(() => {
        if (!open) return;
        openDialogs += 1;
        emit();
        return () => {
            openDialogs -= 1;
            emit();
        };
    }, [open]);
}

export function ModalBackdrop() {
    const active =
        useSyncExternalStore(subscribe, getSnapshot, getSnapshot) > 0;

    return createPortal(
        <div
            aria-hidden="true"
            data-slot="modal-backdrop"
            className={cn(
                "pointer-events-none fixed inset-0 z-40 transition-[background-color,backdrop-filter] duration-200 clip-inset-backdrop",
                active
                    ? "bg-black/32 backdrop-blur-sm"
                    : "bg-transparent backdrop-blur-none"
            )}
        />,
        document.body
    );
}
