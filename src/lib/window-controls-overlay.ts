import { themeStore } from "@/stores/theme-store";

interface WindowControlsOverlayState {
    enabled: boolean;
}

interface WindowControlsOverlayApi {
    ready?: Promise<WindowControlsOverlayState | null>;
    set(options: {
        color?: string;
        symbolColor?: string;
        height?: number;
    }): Promise<WindowControlsOverlayState>;
}

function overlayApi(): WindowControlsOverlayApi | undefined {
    return (
        navigator as unknown as {
            windowControlsOverlay?: WindowControlsOverlayApi;
        }
    ).windowControlsOverlay;
}

type Channels = [number, number, number];

// The plugin parses colors with csscolorparser, which rejects the color() form
// that getComputedStyle returns for color-mix() tokens. Painting one pixel and
// reading it back yields sRGB bytes in a form the parser accepts.
function readSrgb(color: string): Channels | undefined {
    const canvas = document.createElement("canvas");
    canvas.width = 1;
    canvas.height = 1;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    if (!context) return undefined;
    context.fillStyle = color;
    context.fillRect(0, 0, 1, 1);
    const [r, g, b, a] = context.getImageData(0, 0, 1, 1).data;
    if (a === 0) return undefined;
    return [r, g, b];
}

function resolveToken(
    token: string,
    property: "backgroundColor" | "color"
): Channels | undefined {
    const probe = document.createElement("span");
    probe.style[property] = `var(${token})`;
    document.body.append(probe);
    const computed = getComputedStyle(probe)[property];
    probe.remove();
    return readSrgb(computed);
}

let started = false;

export function startWindowControlsOverlay(): void {
    if (started) return;
    const overlay = overlayApi();
    if (!overlay) return;
    started = true;

    const apply = () => {
        const background = resolveToken("--sidebar", "backgroundColor");
        const symbol = resolveToken("--foreground", "color");
        if (!background || !symbol) return;
        void overlay
            .set({
                // Alpha 1, not 0: Win32 lets mouse input through layered windows
                // where alpha is zero, and the band must stay invisible so the
                // inset shadow behind it is not clipped.
                color: `rgba(${background.join(", ")}, 0.004)`,
                symbolColor: `rgb(${symbol.join(", ")})`,
            })
            .catch(() => undefined);
    };

    void overlay.ready?.then((state) => {
        if (!state?.enabled) return;
        apply();
        themeStore.subscribe(apply);
    });
}
