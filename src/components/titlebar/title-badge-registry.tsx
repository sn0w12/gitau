import type { ComponentType } from "react";

import { RepoTitleBadge } from "@/components/repo/repo-title-badge";
import type { TitleBadge } from "@/stores/app-store";

// Bind persisted badge keys to component references here; hydration
// resolves them before first paint after a restart.
const BADGES: Array<[string, ComponentType]> = [["repo", RepoTitleBadge]];

const byKey = new Map<string, ComponentType>(BADGES);
const byComponent = new Map<ComponentType, string>(
    BADGES.map(([key, component]) => [component, key])
);

export function titleBadgeKeyFor(component: ComponentType): string | undefined {
    return byComponent.get(component);
}

export function resolveTitleBadge(
    key: string | undefined
): TitleBadge | undefined {
    if (!key) return undefined;
    const component = byKey.get(key);
    return component ? { key, component } : undefined;
}
