import { getSetting, setSetting } from "@/stores/settings-store";

/**
 * The parent directory remembered across "new repository" and "clone"
 * flows; both write the same hidden setting so alternating between them
 * keeps picking up where the last action happened.
 */
export function lastRepositoryDirectory(): string {
    return getSetting("lastRepositoryDirectory") || "";
}

export function rememberRepositoryDirectory(directory: string): void {
    if (!directory || !directory.trim()) return;
    setSetting("lastRepositoryDirectory", directory);
}
