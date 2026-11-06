import { useEffect, useState } from "react";

import { Scritto } from "@/components/ui/scritto";
import { formatRelativeDate, isRelativeDate } from "@/lib/utils";

const TICK_MS = 30_000;

let timer: ReturnType<typeof setInterval> | null = null;
const listeners = new Set<() => void>();

function tick() {
    for (const listener of listeners) listener();
}

function subscribeToClock(listener: () => void): () => void {
    if (listeners.size === 0) {
        timer = setInterval(tick, TICK_MS);
    }
    listeners.add(listener);
    return () => {
        listeners.delete(listener);
        if (listeners.size === 0 && timer !== null) {
            clearInterval(timer);
            timer = null;
        }
    };
}

/**
 * Keeps a relative timestamp current and rolls its glyphs through Scritto when
 * the label changes. A timestamp old enough to format as a date renders as
 * plain text with no clock subscription, since that label never changes.
 */
export function RelativeTime({ value }: { value: string | number }) {
    const [now, setNow] = useState(Date.now);
    const relative = isRelativeDate(value, now);

    useEffect(() => {
        if (!relative) return;
        return subscribeToClock(() => setNow(Date.now()));
    }, [relative]);

    const label = formatRelativeDate(value, now);
    return relative ? <Scritto value={label} /> : <>{label}</>;
}
