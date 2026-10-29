import {
    MODIFIER_ORDER,
    isModifierKey,
    normalizeKeyName,
} from "@tanstack/hotkeys";
import { useEffect, useState } from "react";

// displayToken (src/lib/shortcuts.ts) keys off the "ctrl" alias, not the
// "Control" name normalizeKeyName produces.
const TOKEN_ALIASES: Record<string, string> = { Control: "Ctrl" };

function chordFromHeldKeys(heldKeys: string[]): string {
    const ordered = [
        ...MODIFIER_ORDER.filter((modifier) => heldKeys.includes(modifier)),
        ...heldKeys.filter((key) => !isModifierKey(key)),
    ];
    return ordered.map((key) => TOKEN_ALIASES[key] ?? key).join("+");
}

export function useHeldChord(active: boolean): string {
    const [heldKeys, setHeldKeys] = useState<string[]>([]);
    const [wasActive, setWasActive] = useState(active);

    if (wasActive !== active) {
        setWasActive(active);
        if (!active) setHeldKeys([]);
    }

    useEffect(() => {
        if (!active) return;

        const handleKeyDown = (event: KeyboardEvent) => {
            const key = normalizeKeyName(event.key);
            setHeldKeys((keys) => (keys.includes(key) ? keys : [...keys, key]));
        };
        const handleKeyUp = (event: KeyboardEvent) => {
            const key = normalizeKeyName(event.key);
            setHeldKeys((keys) => {
                const next = keys.filter((held) => held !== key);
                // Releasing a modifier clears the rest: the OS can swallow the
                // keyup for keys held with it (Cmd+key on macOS).
                return isModifierKey(key)
                    ? next.filter((held) => isModifierKey(held))
                    : next;
            });
        };
        const handleBlur = () => setHeldKeys([]);

        window.addEventListener("keydown", handleKeyDown, true);
        window.addEventListener("keyup", handleKeyUp, true);
        window.addEventListener("blur", handleBlur);
        return () => {
            window.removeEventListener("keydown", handleKeyDown, true);
            window.removeEventListener("keyup", handleKeyUp, true);
            window.removeEventListener("blur", handleBlur);
        };
    }, [active]);

    return active && heldKeys.length > 0 ? chordFromHeldKeys(heldKeys) : "";
}
