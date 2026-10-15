import { useCallback } from "react";
import type { MouseEvent } from "react";

import { useActiveTabRouter } from "@/hooks/tabs/use-active-tab-router";
import { useOpenInNewTab } from "@/hooks/tabs/use-open-in-new-tab";
import { openRepositoryByPath } from "@/lib/repositories/open-repository";
import { toastError } from "@/lib/toast-error";
import { associateTabWithRepoPath, appStore } from "@/stores/app-store";

export interface InternalLinkTarget {
    title: string;
    /** Known in-app route pathname, e.g. "/inbox". */
    href?: string;
    /** Durable repo path, resolved to this process's repoId on click. */
    repoPath?: string;
    /** When set with `repoPath`, the target is a thread inside that repo. */
    thread?: { kind: "issue" | "pull"; number: number };
}

/**
 * An "Open in new tab" intent: Ctrl/Cmd+click, or a middle click. On macOS
 * Ctrl+click is the secondary click, so Cmd is the modifier there.
 */
export function isNewTabClick(
    event: Pick<MouseEvent, "button" | "ctrlKey" | "metaKey">
): boolean {
    return event.button === 1 || event.ctrlKey || event.metaKey;
}

/**
 * Internal navigation as browser-style link props: a plain click moves the
 * active tab, Ctrl/Cmd+click or a middle click opens the target in a new one.
 * Repo and thread targets resolve their repoId on demand, reusing an
 * already-open backend session.
 */
export function useInternalLink() {
    const router = useActiveTabRouter();
    const openInNewTab = useOpenInNewTab();

    const open = useCallback(
        async (target: InternalLinkTarget, newTab: boolean) => {
            if (target.repoPath !== undefined) {
                if (newTab) {
                    await openInNewTab(target);
                    return;
                }
                const opened = await openRepositoryByPath(target.repoPath);
                if (opened.status === "failed") {
                    toastError("Could not open repository", opened.error);
                    return;
                }
                const activeTabId = appStore.state.activeTabId;
                if (activeTabId) {
                    associateTabWithRepoPath(activeTabId, target.repoPath);
                }
                const href = target.thread
                    ? `/repo/${opened.repoId}/${target.thread.kind}/${target.thread.number}`
                    : `/repo/${opened.repoId}`;
                await router?.navigate({ to: href });
                return;
            }

            const href = target.href ?? "/";
            if (newTab) {
                await openInNewTab({ title: target.title, href });
                return;
            }
            await router?.navigate({ to: href });
        },
        [openInNewTab, router]
    );

    return useCallback(
        (target: InternalLinkTarget) => {
            const handle = (event: MouseEvent) => {
                if (event.button !== 0 && event.button !== 1) return;
                const newTab = isNewTabClick(event);
                if (newTab) event.preventDefault();
                void open(target, newTab);
            };
            return { onClick: handle, onAuxClick: handle };
        },
        [open]
    );
}
