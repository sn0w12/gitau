import { useSyncExternalStore } from "react";

export type TimeOfDay = "morning" | "day" | "night";

function getTimeOfDay(date: Date = new Date()): TimeOfDay {
    const hour = date.getHours();
    if (hour >= 5 && hour < 12) return "morning";
    if (hour >= 12 && hour < 22) return "day";
    return "night";
}

const TICK_MS = 60_000;

let period = getTimeOfDay();
let timer: ReturnType<typeof setInterval> | null = null;
const listeners = new Set<() => void>();

function refresh() {
    const next = getTimeOfDay();
    if (next === period) return;
    period = next;
    for (const listener of listeners) listener();
}

function subscribe(listener: () => void): () => void {
    refresh();
    listeners.add(listener);
    timer ??= setInterval(refresh, TICK_MS);
    return () => {
        listeners.delete(listener);
        if (listeners.size === 0 && timer !== null) {
            clearInterval(timer);
            timer = null;
        }
    };
}

function getSnapshot(): TimeOfDay {
    return period;
}

/**
 * Reads one shared period sample for the whole app, so tabs mounted at
 * different times never show different time-of-day artwork. The sample
 * rolls over on the clock boundaries while any tab is subscribed.
 */
export function useTimeOfDay(): TimeOfDay {
    return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}
