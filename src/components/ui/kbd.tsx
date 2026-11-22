import type { RegisterableHotkey } from "@tanstack/react-hotkeys";
import type * as React from "react";
import { createContext, Fragment, useContext } from "react";

import { useSettingValue } from "@/hooks/settings/use-setting";
import {
    SHORTCUT_SETTING_KEYS,
    type ShortcutSettingKey,
} from "@/lib/settings/settings.generated";
import {
    displayToken,
    isActivationToken,
    SHORTCUT_ICON_CLASS,
    splitChord,
} from "@/lib/shortcuts";
import { cn } from "@/lib/utils";

export const MenuShortcutContext = createContext(false);

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
    dataShortcut,
}: {
    shortcut: string[];
    dataShortcut: string;
} & ShortcutProps) {
    const highlightActivationKey = useContext(MenuShortcutContext);
    const lastPart = shortcut.at(-1);
    const activationIndex =
        highlightActivationKey && lastPart && isActivationToken(lastPart)
            ? shortcut.length - 1
            : -1;

    if (shortcut.length === 0) return null;

    return (
        <KbdGroup className={className} data-shortcut={dataShortcut}>
            {shortcut.map((part, index) => {
                const activation = index === activationIndex;
                return (
                    <Fragment key={`${part}-${index}`}>
                        <Kbd
                            className={cn(
                                activation &&
                                    "bg-primary text-primary-foreground",
                                keyClassName
                            )}
                            data-activation={activation ? "true" : undefined}
                        >
                            {displayToken(part)}
                        </Kbd>
                    </Fragment>
                );
            })}
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
    const dataShortcut =
        typeof shortcut === "string" ? shortcut : shortcut.toString();

    if (isShortcutSettingKey(shortcut)) {
        return (
            <SettingKeyboardShortcut
                shortcut={shortcut}
                className={className}
                keyClassName={keyClassName}
                dataShortcut={dataShortcut}
            />
        );
    }

    if (typeof shortcut === "string") {
        return (
            <InternalKeyboardShortcut
                shortcut={splitChord(shortcut)}
                className={className}
                keyClassName={keyClassName}
                dataShortcut={dataShortcut}
            />
        );
    }

    return (
        <HotkeyKeyboardShortcut
            shortcut={shortcut}
            className={className}
            keyClassName={keyClassName}
            dataShortcut={dataShortcut}
        />
    );
}

function HotkeyKeyboardShortcut({
    shortcut,
    className,
    keyClassName,
    dataShortcut,
}: {
    shortcut: RegisterableHotkey;
    dataShortcut: string;
} & ShortcutProps) {
    return (
        <InternalKeyboardShortcut
            shortcut={shortcut.toString().split("+")}
            className={className}
            keyClassName={keyClassName}
            dataShortcut={dataShortcut}
        />
    );
}

function SettingKeyboardShortcut({
    shortcut,
    className,
    keyClassName,
    dataShortcut,
}: {
    shortcut: ShortcutSettingKey;
    dataShortcut: string;
} & ShortcutProps) {
    const chord = useSettingValue(shortcut);
    const parts = splitChord(chord);

    return (
        <InternalKeyboardShortcut
            shortcut={parts}
            className={className}
            keyClassName={keyClassName}
            dataShortcut={dataShortcut}
        />
    );
}
