import { useSelector } from "@tanstack/react-store";

import { pathnameOf } from "@/lib/routing/active-route";
import { appStore, selectActiveTab } from "@/stores/app-store";

export function useActiveTabPathname(): string | null {
    return useSelector(appStore, (state) => {
        const href = selectActiveTab(state)?.lastResolvedHref;
        return href === undefined ? null : pathnameOf(href);
    });
}
