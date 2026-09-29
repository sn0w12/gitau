"use no memo";

import { useParams } from "@tanstack/react-router";
import { useSelector } from "@tanstack/react-store";
import { useEffect, useRef } from "react";

import { BranchSelector } from "@/components/repo/branches/branch-selector";
import { ChangesPanel } from "@/components/repo/changes/changes-panel";
import { HistoryPanel } from "@/components/repo/history/history-panel";
import { RepoViewSwitcher } from "@/components/repo/history/view-switcher";
import { MainRepoView } from "@/components/repo/main-view";
import { RepoFetchButton } from "@/components/repo/sync/fetch-button";
import {
    ResizablePanel,
    ResizablePanelGroup,
    type GroupImperativeHandle,
} from "@/components/ui/resizable";
import { Tabs, TabsContent, TabsList, TabsTab } from "@/components/ui/tabs";
import { useTabId } from "@/contexts/tab-context";
import { repoDisplayName } from "@/hooks/repositories/use-repo-identity";
import { useRepoPageState } from "@/hooks/repositories/use-repo-page-state";
import { useRepoShortcuts } from "@/hooks/repositories/use-repo-shortcuts";
import { useRepository } from "@/hooks/repositories/use-repository-queries";
import { useSettingValue } from "@/hooks/settings/use-setting";
import { useMediaQuery } from "@/hooks/use-media-query";
import { bootMark } from "@/lib/bootstrap/boot-timing";
import { BORDER_GRADIENT } from "@/lib/constants";
import { headBranch, cn } from "@/lib/utils";
import { appStore } from "@/stores/app-store";
import { repositoryStore } from "@/stores/repository-store";
import { setSetting } from "@/stores/settings-store";

// Every tab's repo page reports the same boot metric; only the first
// window-to-content measurement counts.
let reportedRepoReady = false;

export function RepoPage() {
    const tabId = useTabId();
    const params = useParams({ strict: false });
    const repoId = Number(params.repoId);
    const valid = Number.isInteger(repoId) && repoId > 0;

    const entry = useSelector(repositoryStore, (state) => {
        if (!valid) return undefined;
        for (const candidate of state.entries.values()) {
            if (candidate.repoId === repoId) return candidate;
        }
        return undefined;
    });
    const { snapshot, status } = useRepository(valid ? repoId : undefined);
    const lg = useMediaQuery("lg");

    // View, panel, selection, and the issues and pulls list slices live in the
    // query string, so the href a tab was restored with, and the one a back
    // navigation returns to, both reproduce the page.
    const [
        {
            selectedChange,
            selectedCommit,
            selectedStash,
            repoTab,
            view,
            issuesTab,
            pullsTab,
            issueLabel,
            pullLabel,
        },
        dispatch,
    ] = useRepoPageState(repoId);

    useEffect(() => {
        if (reportedRepoReady) return;
        if (snapshot.isSuccess && status.isSuccess) {
            reportedRepoReady = true;
            bootMark("repo.ready");
        }
    }, [snapshot.isSuccess, status.isSuccess]);

    const tab = useSelector(appStore, (state) =>
        state.tabs.find((candidate) => candidate.tabId === tabId)
    );

    const name = repoDisplayName(entry?.path ?? tab?.repoPath ?? "");
    const head = snapshot.data?.head;
    const branch = headBranch(head);
    const repoPath = entry?.path ?? tab?.repoPath ?? "";

    const containerRef = useRef<HTMLDivElement>(null);
    useRepoShortcuts({
        repoId: valid ? repoId : undefined,
        repoPath,
        view,
        dispatch,
        containerRef,
        enabled: valid && entry !== undefined,
    });

    const sidebarSize = useSettingValue("repoSidebarSize");
    const sidebarGroupRef = useRef<GroupImperativeHandle | null>(null);
    // Keeps every open repo tab at the shared sidebar width. Drags in this
    // group write the setting themselves, so this only reacts to other tabs.
    useEffect(() => {
        sidebarGroupRef.current?.setLayout({
            "repo-sidebar": sidebarSize,
            "repo-main": 100 - sidebarSize,
        });
    }, [sidebarSize]);

    if (!valid || !entry) {
        return (
            <div className="flex h-full items-center justify-center p-6">
                <p className="text-sm text-muted-foreground">
                    Repository not open in this session.
                </p>
            </div>
        );
    }

    return (
        <div ref={containerRef} className="flex size-full max-h-full flex-col">
            <ResizablePanelGroup
                groupRef={sidebarGroupRef}
                onLayoutChanged={(layout, meta) => {
                    if (!meta.isUserInteraction) return;
                    const rounded =
                        Math.round(layout["repo-sidebar"] * 10) / 10;
                    if (rounded !== sidebarSize) {
                        setSetting("repoSidebarSize", rounded);
                    }
                }}
            >
                <ResizablePanel
                    id="repo-sidebar"
                    className="flex h-full min-w-0 flex-col"
                    style={{ overflow: "hidden" }}
                    defaultSize={sidebarSize}
                    minSize={lg ? "20%" : "35%"}
                    maxSize={lg ? "35%" : "50%"}
                >
                    <div
                        className={cn(
                            "ui-selectable flex h-14 flex-row items-center justify-between gap-3 border-b px-2",
                            BORDER_GRADIENT
                        )}
                    >
                        <span className="truncate pt-0.5 font-display text-4xl text-trim-both">
                            {name}
                        </span>
                        <div className="h-full flex-1" />
                    </div>
                    <Tabs
                        className="min-h-0 flex-1 gap-0 border-r"
                        value={repoTab}
                        onValueChange={(value) => {
                            dispatch({ type: "SET_TAB", data: value });
                        }}
                    >
                        <div className="p-0.5">
                            <TabsList className="w-full">
                                <TabsTab value="changes">Changes</TabsTab>
                                <TabsTab value="history">History</TabsTab>
                            </TabsList>
                        </div>
                        <TabsContent
                            className="min-h-0 text-sm"
                            value="changes"
                            keepMounted
                        >
                            <ChangesPanel
                                repoId={repoId}
                                branch={branch}
                                selectedId={selectedChange}
                                onSelectedChange={(id) => {
                                    dispatch({ type: "SET_CHANGE", data: id });
                                }}
                                selectedStashId={selectedStash}
                                onSelectedStash={(id) => {
                                    dispatch({ type: "SET_STASH", data: id });
                                }}
                            />
                        </TabsContent>
                        <TabsContent
                            className="min-h-0 text-sm"
                            value="history"
                            keepMounted
                        >
                            <HistoryPanel
                                repoId={repoId}
                                active={repoTab === "history"}
                                selectedId={selectedCommit}
                                onSelect={(id) => {
                                    dispatch({ type: "SET_COMMIT", data: id });
                                }}
                            />
                        </TabsContent>
                    </Tabs>
                </ResizablePanel>
                <ResizablePanel
                    id="repo-main"
                    className="min-w-0"
                    style={{ overflow: "hidden" }}
                >
                    <div className="flex size-full min-w-0 flex-col">
                        <div className="flex h-14 min-h-14 items-center">
                            <BranchSelector repoId={repoId} />
                            <RepoFetchButton
                                repoId={repoId}
                                repoPath={repoPath}
                            />
                            <RepoViewSwitcher
                                view={view}
                                onView={(next) =>
                                    dispatch({ type: "SET_VIEW", data: next })
                                }
                            />
                            <div className="h-full flex-1 border-b" />
                        </div>
                        <MainRepoView
                            repoId={repoId}
                            tab={repoTab}
                            view={view}
                            issuesTab={issuesTab}
                            pullsTab={pullsTab}
                            issueLabel={issueLabel}
                            pullLabel={pullLabel}
                            generation={snapshot.data?.generation}
                            selectedChangeId={selectedChange}
                            selectedCommitId={selectedCommit}
                            selectedStashId={selectedStash}
                            dispatch={dispatch}
                        />
                    </div>
                </ResizablePanel>
            </ResizablePanelGroup>
        </div>
    );
}
