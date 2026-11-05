// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { MainRepoView } from "@/components/repo/main-view";
import { TabContext } from "@/contexts/tab-context";
import type { RepoTab, RepoView } from "@/lib/routing/repo-search";

vi.mock("@/components/repo/readme/readme-view", () => ({
    ReadmeView: () => <div>readme</div>,
}));
vi.mock("@/components/diff/commit-diff-view", () => ({
    CommitDiffView: ({ commitId }: { commitId: string }) => (
        <div>commit {commitId}</div>
    ),
}));
vi.mock("@/components/repo/history/commit-graph-view", () => ({
    CommitGraphView: () => null,
}));
vi.mock("@/components/repo/history/history-chart-view", () => ({
    HistoryChartView: () => null,
}));
vi.mock("@/components/repo/issues/issues-view", () => ({
    IssuesView: () => null,
}));
vi.mock("@/components/repo/pulls/pulls-view", () => ({
    PullRequestsView: () => null,
}));

function renderView({
    view,
    tab,
    selectedCommitId = null,
}: {
    view: RepoView;
    tab: RepoTab;
    selectedCommitId?: string | null;
}) {
    return render(
        <TabContext.Provider value="tab-1">
            <MainRepoView
                repoId={1}
                tab={tab}
                view={view}
                issuesTab="open"
                pullsTab="open"
                issueLabel="none"
                pullLabel="none"
                generation={1}
                selectedChangeId={null}
                selectedCommitId={selectedCommitId}
                selectedStashId={null}
                dispatch={() => {}}
            />
        </TabContext.Provider>
    );
}

describe("MainRepoView", () => {
    afterEach(() => {
        cleanup();
    });

    it("shows the readme when nothing is selected", () => {
        renderView({ view: "readme", tab: "history" });
        expect(screen.getByText("readme")).toBeTruthy();
    });

    it("shows the selected commit over the readme", () => {
        renderView({
            view: "readme",
            tab: "history",
            selectedCommitId: "abc123",
        });
        expect(screen.getByText("commit abc123")).toBeTruthy();
        expect(screen.queryByText("readme")).toBeNull();
    });

    it("keeps the readme for a commit selected outside the history tab", () => {
        renderView({
            view: "readme",
            tab: "changes",
            selectedCommitId: "abc123",
        });
        expect(screen.getByText("readme")).toBeTruthy();
        expect(screen.queryByText("commit abc123")).toBeNull();
    });
});
