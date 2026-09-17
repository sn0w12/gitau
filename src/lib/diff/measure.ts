import { prepareWithSegments, measureNaturalWidth } from "@chenglou/pretext";

/**
 * The diff rows render in `font-mono text-xs` = 0.75rem = 12px Geist Mono
 * (see styles.css). Pretext measures through the browser's own font engine,
 * so this returns the real advance width instead of a guessed average.
 */
const DIFF_FONT = '12px "Geist Mono Variable", monospace';

/** Used when canvas measurement is unavailable (tests, SSR). */
const FALLBACK_CHAR_WIDTH = 7.25;

const SAMPLE_CHARS = 64;

let cachedWidth: number | null = null;
const listeners = new Set<() => void>();

function notify(): void {
    listeners.forEach((listener) => listener());
}

function measureOnce(): number | null {
    try {
        if (typeof document === "undefined") return null;
        const prepared = prepareWithSegments(
            "M".repeat(SAMPLE_CHARS),
            DIFF_FONT
        );
        const total = measureNaturalWidth(prepared);
        if (Number.isFinite(total) && total > 0) {
            return total / SAMPLE_CHARS;
        }
        return null;
    } catch {
        return null;
    }
}

/**
 * Best known advance width of one diff-font character, in CSS pixels:
 * monospace glyphs share one advance, so a single sampled measurement is
 * exact and lets pretext's cache serve every call.
 */
export function diffCharWidth(): number {
    return cachedWidth ?? FALLBACK_CHAR_WIDTH;
}

export function subscribeDiffCharWidth(listener: () => void): () => void {
    listeners.add(listener);
    return () => {
        listeners.delete(listener);
    };
}

/**
 * Re-measures when the bundled mono font becomes available to canvas;
 * measuring earlier silently reports fallback system-mono metrics, which
 * under-measure Geist Mono and make rows wrap too early.
 */
export function watchDiffCharWidth(): () => void {
    if (typeof document === "undefined") return () => undefined;
    const fonts = document.fonts as FontFaceSet | undefined;
    if (!fonts) return () => undefined;

    let active = true;
    const remeasure = () => {
        if (!active) return;
        const next = measureOnce();
        if (
            next !== null &&
            Math.abs(next - (cachedWidth ?? Number.NaN)) > 1e-6
        ) {
            cachedWidth = next;
            notify();
        }
    };

    // An immediate measurement is only trustworthy if the face is already
    // loaded; otherwise wait for our explicit load request to settle.
    try {
        if (fonts.check(DIFF_FONT)) remeasure();
    } catch {
        // check() can throw for unparseable fonts on odd platforms.
    }

    const request =
        typeof fonts.load === "function"
            ? fonts.load(DIFF_FONT).catch(() => undefined)
            : Promise.resolve();
    Promise.resolve(request)
        .then(() => fonts.ready)
        .then(() => remeasure())
        .catch(() => undefined);

    return () => {
        active = false;
    };
}
