import type { RegisterableHotkey } from "@tanstack/react-hotkeys";
import type * as React from "react";
import { Fragment } from "react";

import { useSettingValue } from "@/hooks/settings/use-setting";
import {
    SHORTCUT_SETTING_KEYS,
    type ShortcutSettingKey,
} from "@/lib/settings/settings.generated";
import { displayToken, SHORTCUT_ICON_CLASS, splitChord } from "@/lib/shortcuts";
import { cn } from "@/lib/utils";

export function Kbd({
    className,
    ...props
}: React.ComponentProps<"kbd">): React.ReactElement {
    return (
        <kbd
            className={cn(
                "pointer-events-none inline-flex h-5 min-w-5 items-center justify-center gap-1 rounded-[.25rem] bg-muted px-1 text-xs font-medium text-muted-foreground select-none",
                SHORTCUT_ICON_CLASS,
                className
            )}
            data-slot="kbd"
            {...props}
        />
    );
}

export function KbdGroup({
    className,
    ...props
}: React.ComponentProps<"kbd">): React.ReactElement {
    return (
        <kbd
            className={cn("inline-flex items-center gap-1", className)}
            data-slot="kbd-group"
            {...props}
        />
    );
}

function InternalKeyboardShortcut({
    shortcut,
    className,
    keyClassName,
}: {
    shortcut: string[];
} & ShortcutProps) {
    if (shortcut.length === 0) return null;

    return (
        <KbdGroup className={className}>
            {shortcut.map((part, index) => (
                <Fragment key={`${part}-${index}`}>
                    <Kbd className={keyClassName}>{displayToken(part)}</Kbd>
                </Fragment>
            ))}
        </KbdGroup>
    );
}

function isShortcutSettingKey(value: unknown): value is ShortcutSettingKey {
    return (
        typeof value === "string" &&
        SHORTCUT_SETTING_KEYS.includes(value as ShortcutSettingKey)
    );
}

type StringChord = `${string}+${string}`;
export type AnyShortcut = RegisterableHotkey | ShortcutSettingKey | StringChord;
interface ShortcutProps {
    className?: string;
    keyClassName?: string;
}

export function KeyboardShortcut({
    shortcut,
    className,
    keyClassName,
}: {
    shortcut: AnyShortcut;
} & ShortcutProps) {
    if (isShortcutSettingKey(shortcut)) {
        return (
            <SettingKeyboardShortcut
                shortcut={shortcut}
                className={className}
                keyClassName={keyClassName}
            />
        );
    }

    if (typeof shortcut === "string") {
        return (
            <InternalKeyboardShortcut
                shortcut={splitChord(shortcut)}
                className={className}
                keyClassName={keyClassName}
            />
        );
    }

    return (
        <HotkeyKeyboardShortcut
            shortcut={shortcut}
            className={className}
            keyClassName={keyClassName}
        />
    );
}

function HotkeyKeyboardShortcut({
    shortcut,
    className,
    keyClassName,
}: {
    shortcut: RegisterableHotkey;
} & ShortcutProps) {
    return (
        <InternalKeyboardShortcut
            shortcut={shortcut.toString().split("+")}
            className={className}
            keyClassName={keyClassName}
        />
    );
}

function SettingKeyboardShortcut({
    shortcut,
    className,
    keyClassName,
}: {
    shortcut: ShortcutSettingKey;
} & ShortcutProps) {
    const chord = useSettingValue(shortcut);
    const parts = splitChord(chord);

    return (
        <InternalKeyboardShortcut
            shortcut={parts}
            className={className}
            keyClassName={keyClassName}
        />
    );
}
