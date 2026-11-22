import { useEffect } from "react";

const KEYBOARD_INPUT_KEYS = new Set([
    "Tab",
    "Enter",
    " ",
    "ArrowUp",
    "ArrowDown",
    "ArrowLeft",
    "ArrowRight",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "ContextMenu",
    "F10",
]);

/**
 * Records the last input modality on `<html data-input-modality>` so styles can
 * drop focus rings after pointer use while keeping them for keyboard
 * navigation. Dismissal keys such as Escape and single-character shortcuts do
 * not count as navigation.
 */
export function useInputModality(): void {
    useEffect(() => {
        const root = document.documentElement;
        root.dataset.inputModality = "keyboard";

        const markPointer = () => {
            root.dataset.inputModality = "pointer";
        };

        const markKeyboard = (event: KeyboardEvent) => {
            if (event.isComposing) return;
            if (event.ctrlKey || event.metaKey || event.altKey) return;
            if (!KEYBOARD_INPUT_KEYS.has(event.key)) return;
            root.dataset.inputModality = "keyboard";
        };

        window.addEventListener("pointerdown", markPointer, true);
        window.addEventListener("keydown", markKeyboard, true);
        return () => {
            window.removeEventListener("pointerdown", markPointer, true);
            window.removeEventListener("keydown", markKeyboard, true);
            delete root.dataset.inputModality;
        };
    }, []);
}
