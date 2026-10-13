import { useSelector } from "@tanstack/react-store";

import { pathnameOf } from "@/lib/routing/active-route";
import { appStore, selectActiveTab } from "@/stores/app-store";

export function useActiveTabPathname(): string | null {
    const href = useSelector(
        appStore,
        (state) => selectActiveTab(state)?.lastResolvedHref ?? null
    );
    return href === null ? null : pathnameOf(href);
}
