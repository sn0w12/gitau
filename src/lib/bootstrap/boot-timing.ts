import { info } from "@tauri-apps/plugin-log";

const MARKER = "[boottrace]";
const WEBVIEW_PID = 2;

const LANES = new Map<string, number>();

function lane(name: string): number {
    const existing = LANES.get(name);
    if (existing !== undefined) return existing;
    const next = LANES.size + 1;
    LANES.set(name, next);
    return next;
}

// performance.timeOrigin is the wall-clock epoch at navigation start, so
// adding the monotonic clock yields stamps that line up with the Rust spans.
function nowMicros(): number {
    return Math.round((performance.timeOrigin + performance.now()) * 1000);
}

function emit(event: Record<string, unknown>): void {
    // The log transport is absent outside the Tauri webview (tests); boot
    // timing must never turn into an unhandled rejection there.
    info(`${MARKER} ${JSON.stringify(event)}`).catch(() => undefined);
}

export interface BootSpan {
    end(): void;
}

export function bootSpan(name: string): BootSpan {
    const started = nowMicros();
    let ended = false;
    return {
        end() {
            if (ended) return;
            ended = true;
            emit({
                name,
                cat: "boot",
                ph: "X",
                ts: started,
                dur: nowMicros() - started,
                pid: WEBVIEW_PID,
                tid: lane(name),
            });
        },
    };
}

export function bootMark(name: string): void {
    emit({
        name,
        cat: "boot",
        ph: "i",
        s: "g",
        ts: nowMicros(),
        pid: WEBVIEW_PID,
        tid: lane(name),
    });
}
