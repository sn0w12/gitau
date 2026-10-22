import { getCurrentWindow } from "@tauri-apps/api/window";

import { isMac } from "@/lib/utils";

let simpleFullscreen = false;

export async function toggleFullscreen(): Promise<void> {
    if (!isMac()) {
        const win = getCurrentWindow();
        await win.setFullscreen(!(await win.isFullscreen()));
        return;
    }
    // isFullscreen() stays false in simple fullscreen, so it cannot drive the
    // toggle; the value is tracked here instead.
    simpleFullscreen = !simpleFullscreen;
    await getCurrentWindow().setSimpleFullscreen(simpleFullscreen);
}
