import type { Dispatch, RefObject } from "react";

import { useSettingHotkey } from "@/hooks/settings/use-setting-hotkey";
import type { reducerAction, RepoView } from "@/routes/repo-page";

import { useChangeActions } from "./use-change-actions";
import { useSyncActions } from "./use-sync-actions";

interface RepoShortcutScope {
    repoId: number | undefined;
    repoPath: string;
    view: RepoView;
    dispatch: Dispatch<reducerAction>;
    containerRef: RefObject<HTMLDivElement | null>;
    enabled: boolean;
}

/**
 * Repo-scoped shortcuts. Registered inside the repo page so they arm only
 * while that tab's view is visible; hidden tabs drop their registrations
 * through the Activity teardown.
 */
export function useRepoShortcuts({
    repoId,
    repoPath,
    view,
    dispatch,
    containerRef,
    enabled,
}: RepoShortcutScope): void {
    const changes = useChangeActions(repoId);
    const sync = useSyncActions(repoId ?? 0, repoPath);

    // Tab panels stay mounted, so the target exists before the switch
    // paints; focusing is deferred one frame so it lands when visible.
    const focusInChangesPanel = (selector: string) => {
        dispatch({ type: "SET_TAB", data: "changes" });
        requestAnimationFrame(() => {
            containerRef.current?.querySelector<HTMLElement>(selector)?.focus();
        });
    };

    useSettingHotkey("stageAllShortcut", () => void changes.stageAll(), {
        enabled,
    });
    useSettingHotkey("unstageAllShortcut", () => void changes.unstageAll(), {
        enabled,
    });
    useSettingHotkey("discardAllShortcut", () => void changes.discardAll(), {
        enabled,
    });
    useSettingHotkey("fetchShortcut", () => void sync.fetch(), { enabled });
    useSettingHotkey("pullShortcut", () => void sync.pull(), { enabled });
    useSettingHotkey("pushShortcut", () => void sync.push(), { enabled });
    useSettingHotkey(
        "switchChangesPanelShortcut",
        () => dispatch({ type: "SET_TAB", data: "changes" }),
        { enabled }
    );
    useSettingHotkey(
        "switchHistoryPanelShortcut",
        () => dispatch({ type: "SET_TAB", data: "history" }),
        { enabled }
    );
    useSettingHotkey(
        "toggleGraphViewShortcut",
        () =>
            dispatch({
                type: "SET_VIEW",
                data: view === "graph" ? "overview" : "graph",
            }),
        { enabled }
    );
    useSettingHotkey(
        "filterChangesShortcut",
        () => focusInChangesPanel("#changes-filter-input"),
        { enabled }
    );
    useSettingHotkey(
        "focusCommitSummaryShortcut",
        () => focusInChangesPanel("#commit-summary-input"),
        { enabled }
    );
}
