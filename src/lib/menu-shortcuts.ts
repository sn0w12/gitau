import {
    SHORTCUT_SETTING_KEYS,
    type ShortcutSettingKey,
} from "@/lib/settings/settings.generated";
import { isActivationToken, splitChord } from "@/lib/shortcuts";
import { getSetting } from "@/stores/settings-store";

interface MenuShortcutKeyEvent {
    key: string;
    repeat?: boolean;
    isComposing?: boolean;
    ctrlKey?: boolean;
    altKey?: boolean;
    metaKey?: boolean;
    target: EventTarget | null;
    currentTarget: HTMLElement;
    preventDefault: () => void;
    stopPropagation: () => void;
}

function isShortcutSettingKey(value: string): value is ShortcutSettingKey {
    return SHORTCUT_SETTING_KEYS.includes(value as ShortcutSettingKey);
}

function resolveChord(shortcut: string): string {
    return isShortcutSettingKey(shortcut) ? getSetting(shortcut) : shortcut;
}

function activationKey(chord: string): string | undefined {
    const token = splitChord(chord).at(-1);
    return token && isActivationToken(token) ? token.toLowerCase() : undefined;
}

function isEditable(target: EventTarget | null): boolean {
    return (
        target instanceof HTMLElement &&
        (target.isContentEditable ||
            Boolean(target.closest("input, textarea, select")))
    );
}

/**
 * Runs the menu item whose displayed shortcut ends in the pressed character,
 * so an open menu can be driven without its modifier keys. Returns true when
 * the key was consumed.
 */
export function activateMenuShortcut(event: MenuShortcutKeyEvent): boolean {
    if (event.repeat || event.isComposing) return false;
    if (event.ctrlKey || event.altKey || event.metaKey) return false;
    if (Array.from(event.key).length !== 1 || event.key === " ") return false;
    if (isEditable(event.target)) return false;
    if (
        !(event.target instanceof Node) ||
        !event.currentTarget.contains(event.target)
    ) {
        return false;
    }

    const key = event.key.toLowerCase();
    const matches = new Set<HTMLElement>();
    for (const badge of event.currentTarget.querySelectorAll<HTMLElement>(
        "[data-shortcut]"
    )) {
        if (activationKey(resolveChord(badge.dataset.shortcut ?? "")) !== key) {
            continue;
        }
        const item = badge.closest<HTMLElement>('[role^="menuitem"]');
        if (
            !item ||
            item.hasAttribute("data-disabled") ||
            item.getAttribute("aria-disabled") === "true"
        ) {
            continue;
        }
        matches.add(item);
    }

    if (matches.size !== 1) return false;

    event.preventDefault();
    event.stopPropagation();
    matches.values().next().value?.click();
    return true;
}
