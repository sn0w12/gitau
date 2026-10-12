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

function wallMicros(perfMs: number): number {
    return Math.round((performance.timeOrigin + perfMs) * 1000);
}

function emitSpanAt(name: string, startPerf: number, endPerf: number): void {
    if (!(endPerf >= startPerf)) return;
    emit({
        name,
        cat: "boot",
        ph: "X",
        ts: wallMicros(startPerf),
        dur: Math.round((endPerf - startPerf) * 1000),
        pid: WEBVIEW_PID,
        tid: lane(name),
    });
}

function emitInstantAt(name: string, perfMs: number): void {
    emit({
        name,
        cat: "boot",
        ph: "i",
        s: "g",
        ts: wallMicros(perfMs),
        pid: WEBVIEW_PID,
        tid: lane(name),
    });
}

export function bootNavigationTimings(): void {
    const nav = performance.getEntriesByType("navigation")[0] as
        | PerformanceNavigationTiming
        | undefined;
    if (!nav || nav.responseStart <= 0) return;
    emitSpanAt("webview.nav.request", nav.startTime, nav.responseStart);
    emitSpanAt("webview.nav.response", nav.responseStart, nav.responseEnd);
    emitSpanAt("webview.nav.parse", nav.responseEnd, nav.domInteractive);
    emitSpanAt(
        "webview.nav.moduleExec",
        nav.domInteractive,
        nav.domContentLoadedEventStart
    );
    emitSpanAt(
        "webview.nav.domContentLoaded",
        nav.domContentLoadedEventStart,
        nav.domContentLoadedEventEnd
    );
    emitSpanAt("webview.nav.load", nav.loadEventStart, nav.loadEventEnd);
    emitInstantAt("webview.nav.domInteractive", nav.domInteractive);
    emitInstantAt("webview.nav.domContentLoaded", nav.domContentLoadedEventEnd);
    emitInstantAt("webview.nav.complete", nav.loadEventEnd);
}

export function bootPaintTimings(): void {
    if (typeof PerformanceObserver === "undefined") return;
    new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
            emitInstantAt(`webview.paint ${entry.name}`, entry.startTime);
        }
    }).observe({ type: "paint", buffered: true });
}

export function bootLongTasks(): void {
    if (typeof PerformanceObserver === "undefined") return;
    new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
            emitSpanAt(
                "webview.longtask",
                entry.startTime,
                entry.startTime + entry.duration
            );
        }
    }).observe({ type: "longtask", buffered: true });
}

interface FrameScript {
    invoker: string;
    sourceURL: string;
    sourceFunctionName: string;
    startTime: number;
    duration: number;
    forcedStyleAndLayoutDuration: number;
}

export function bootLongFrames(): void {
    if (typeof PerformanceObserver === "undefined") return;
    const observer = new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
            emitSpanAt(
                "webview.frame",
                entry.startTime,
                entry.startTime + entry.duration
            );
            const scripts =
                (entry as unknown as { scripts?: FrameScript[] }).scripts ?? [];
            for (const script of scripts) {
                const source =
                    script.sourceURL.split("/").pop() || script.sourceURL;
                const name = script.sourceFunctionName || script.invoker || "?";
                emitSpanAt(
                    `webview.frame.script ${source} via=${script.invoker} fn=${name} layout=${Math.round(script.forcedStyleAndLayoutDuration)}ms`,
                    entry.startTime + script.startTime,
                    entry.startTime + script.startTime + script.duration
                );
            }
        }
    });
    try {
        observer.observe({
            type: "long-animation-frame",
            buffered: true,
        } as unknown as PerformanceObserverInit);
    } catch {
        // Engine without LoAF support; the longtask observer still reports.
    }
}

function shortResource(url: string): string {
    const path = url.split("?")[0].split("#")[0];
    const parts = path.split("/").filter(Boolean);
    return parts.slice(-2).join("/") || path;
}

export function bootResourceTimings(): void {
    const entries = performance.getEntriesByType(
        "resource"
    ) as PerformanceResourceTiming[];
    for (const entry of entries) {
        if (
            entry.responseEnd <= entry.startTime ||
            entry.name.startsWith("data:")
        ) {
            continue;
        }
        emitSpanAt(
            `webview.resource ${shortResource(entry.name)}`,
            entry.startTime,
            entry.responseEnd
        );
    }
}
