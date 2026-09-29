import { describe, expect, it } from "vitest";

import {
    DEFAULT_REPO_STATE,
    type RepoState,
    parseRepoSearch,
    repoSearchParams,
    repoStateReducer,
    validateRepoSearch,
} from "@/lib/routing/repo-search";

function state(overrides: Partial<RepoState> = {}): RepoState {
    return { ...DEFAULT_REPO_STATE, ...overrides };
}

describe("parseRepoSearch", () => {
    it("reads every accepted value", () => {
        expect(
            parseRepoSearch({
                view: "graph",
                tab: "history",
                change: "worktree:a.ts",
                commit: "abc",
                stash: "def",
                issuesTab: "closed",
                pullsTab: "closed",
                issueLabel: "bug",
                pullLabel: "chore",
            })
        ).toEqual(
            state({
                view: "graph",
                repoTab: "history",
                selectedChange: "worktree:a.ts",
                selectedCommit: "abc",
                selectedStash: "def",
                issuesTab: "closed",
                pullsTab: "closed",
                issueLabel: "bug",
                pullLabel: "chore",
            })
        );
    });

    it("degrades junk to the defaults and keeps the two lists independent", () => {
        expect(
            parseRepoSearch({
                view: "tree",
                tab: "files",
                change: 42,
                commit: "",
                issuesTab: "all",
                issueLabel: "",
            })
        ).toEqual(DEFAULT_REPO_STATE);

        const onlyIssues = parseRepoSearch({ issuesTab: "closed" });
        expect(onlyIssues.pullsTab).toBe("open");
    });
});

describe("repoStateReducer", () => {
    it("keeps one selection at a time and drops it on a tab, view, or clear", () => {
        const afterStash = repoStateReducer(
            state({ selectedChange: "index:a.ts", selectedCommit: "abc" }),
            { type: "SET_STASH", data: "def" }
        );
        expect(afterStash).toEqual(state({ selectedStash: "def" }));

        const afterChange = repoStateReducer(afterStash, {
            type: "SET_CHANGE",
            data: "index:a.ts",
        });
        expect(afterChange.selectedStash).toBeNull();

        expect(
            repoStateReducer(afterChange, { type: "SET_TAB", data: "history" })
        ).toEqual(state({ repoTab: "history" }));
        expect(
            repoStateReducer(afterChange, { type: "SET_VIEW", data: "graph" })
        ).toEqual(state({ view: "graph" }));
        expect(
            repoStateReducer(afterChange, { type: "CLEAR_SELECTION" })
        ).toEqual(DEFAULT_REPO_STATE);
    });

    it("keeps the list slices across a view switch", () => {
        const onIssues = state({ view: "issues", issuesTab: "closed" });
        const onGraph = repoStateReducer(onIssues, {
            type: "SET_VIEW",
            data: "graph",
        });
        expect(onGraph.issuesTab).toBe("closed");
        expect(
            repoStateReducer(onGraph, { type: "SET_VIEW", data: "issues" })
                .issuesTab
        ).toBe("closed");
    });

    // The identity is what lets dispatch skip a navigation, and a redundant
    // navigate leaves the href looking right while still costing a reload of
    // the route loader.
    it("hands back the same object for a no-op action", () => {
        const current = state({
            view: "graph",
            selectedCommit: "abc",
            issuesTab: "closed",
        });

        expect(
            repoStateReducer(current, { type: "SET_VIEW", data: "graph" })
        ).toBe(current);
        expect(
            repoStateReducer(current, { type: "SET_COMMIT", data: "abc" })
        ).toBe(current);
        expect(
            repoStateReducer(current, {
                type: "SET_ISSUES_TAB",
                data: "closed",
            })
        ).toBe(current);
    });
});

describe("the search string", () => {
    it("leaves the defaults implicit and round-trips everything else", () => {
        expect(repoSearchParams(DEFAULT_REPO_STATE)).toEqual({});

        const full = state({
            view: "issues",
            repoTab: "history",
            selectedChange: "conflict:src/a.ts",
            issuesTab: "closed",
            pullsTab: "closed",
            issueLabel: "bug",
            pullLabel: "chore",
        });
        expect(parseRepoSearch(repoSearchParams(full))).toEqual(full);
    });

    // Idempotence is what stops the router rewriting the href under itself on
    // every navigation, and unknown keys are what would otherwise survive.
    it("validates to a fixed point", () => {
        expect(
            validateRepoSearch({
                view: "pulls",
                tab: "history",
                change: "worktree:a.ts",
                repoId: "7",
            })
        ).toEqual({ view: "pulls", tab: "history", change: "worktree:a.ts" });

        const once = validateRepoSearch({
            view: "graph",
            change: "index:a.ts",
        });
        expect(validateRepoSearch(once)).toEqual(once);
    });
});
