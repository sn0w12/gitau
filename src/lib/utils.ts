import { platform } from "@tauri-apps/plugin-os";
import { clsx } from "clsx";
import type { ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

import type { HeadState } from "@/lib/backend/protocol";

export function cn(...inputs: ClassValue[]): string {
    return twMerge(clsx(inputs));
}

/** Formats the HEAD state for display: branch name or detached marker. */
export function headBranch(head: HeadState | undefined): string {
    if (!head) return "…";
    if (head.state === "attached") return head.branch;
    if (head.state === "unborn") return head.branch;
    return "detached HEAD";
}

export function pluralize(word: string, count: number) {
    return count === 1 ? word : `${word}s`;
}

export function isMac(): boolean {
    try {
        return platform() === "macos";
    } catch (e) {
        console.error("platform check failed", e);
        return navigator.userAgent.toLowerCase().includes("mac");
    }
}

export function isWindows(): boolean {
    try {
        return platform() === "windows";
    } catch (e) {
        console.error("platform check failed", e);
        return navigator.userAgent.toLowerCase().includes("win");
    }
}

/** OS-native name of the trash destination for destructive flows. */
export function trashBinName(): string {
    return isWindows() ? "Recycle Bin" : "Trash";
}

export function formatRelativeDate(dateString: string | number): string {
    const date = new Date(dateString);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMinutes = Math.floor(diffMs / (1000 * 60));
    const diffHours = Math.floor(diffMs / (1000 * 60 * 60));
    const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

    if (diffMinutes < 60) {
        if (diffMinutes < 1) {
            return "now";
        }
        if (diffMinutes === 1) {
            return `1 minute ago`;
        }
        return `${diffMinutes} minutes ago`;
    } else if (diffHours < 24) {
        if (diffHours === 1) {
            return `1 hour ago`;
        }
        return `${diffHours} hours ago`;
    } else if (diffDays < 7) {
        if (diffDays === 1) {
            return `1 day ago`;
        }
        return `${diffDays} days ago`;
    } else {
        const options: Intl.DateTimeFormatOptions = {
            year: "numeric",
            month: "long",
            day: "numeric",
        };
        return date.toLocaleDateString("en-US", options);
    }
}

type TextColor = "bright" | "dark";

/**
 * Determines whether bright or dark text should be used on a given background color.
 */
export function getTextColor(hex: string): TextColor {
    // Strip the leading "#" if present
    let value = hex.startsWith("#") ? hex.slice(1) : hex;
    if (value.length === 3) {
        value = value
            .split("")
            .map((c) => c + c)
            .join("");
    }

    if (!/^[0-9a-fA-F]{6}$/.test(value)) {
        throw new Error(`Invalid hex color string: "${hex}"`);
    }

    const r = parseInt(value.slice(0, 2), 16);
    const g = parseInt(value.slice(2, 4), 16);
    const b = parseInt(value.slice(4, 6), 16);

    const yiq = (r * 299 + g * 587 + b * 114) / 1000;
    return yiq >= 128 ? "dark" : "bright";
}
