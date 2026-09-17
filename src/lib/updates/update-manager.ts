import { createStore } from "@tanstack/store";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";

import { UpdateController } from "./update-controller";

/**
 * Process-wide updater singleton. Both the titlebar install button and the
 * update dialog subscribe to the same controller, and the dialog-open flag
 * is what turns a button click into a visible dialog.
 */
export const updaterController = new UpdateController({
    check: () => check(),
    relaunch: () => relaunch(),
});

export const updateDialogOpenStore = createStore(false);

export function openUpdateDialog(): void {
    updateDialogOpenStore.setState(() => true);
}

export function closeUpdateDialog(): void {
    updateDialogOpenStore.setState(() => false);
}
