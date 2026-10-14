import { useCallback } from "react";

import { openRepositoryByPath } from "@/lib/repositories/open-repository";
import { toastError } from "@/lib/toast-error";
import { createTabRecord, openTab } from "@/stores/app-store";

interface OpenInNewTabTarget {
    title: string;
    href?: string;
    repoPath?: string;
}

/**
 * Opens a target in a fresh tab. A repo target opens its backend session
 * first so the new tab's href carries this process's repoId; a route target
 * needs no resolution.
 */
export function useOpenInNewTab() {
    return useCallback(async (target: OpenInNewTabTarget) => {
        let href = target.href ?? "/";
        if (target.repoPath !== undefined) {
            const opened = await openRepositoryByPath(target.repoPath);
            if (opened.status === "failed") {
                toastError("Could not open repository", opened.error);
                return;
            }
            href = `/repo/${opened.repoId}`;
        }

        openTab(
            createTabRecord({
                title: target.title,
                repoPath: target.repoPath,
                initialHref: href,
            })
        );
    }, []);
}
