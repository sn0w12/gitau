/**
 * Repo page state lives in the query string, not in component state, so a tab
 * restored in a new process, a reload, and a back out of an issue page all land
 * on the same view, panel, selection, and issues/pulls list slice. This module
 * owns the whole contract: the accepted value lists, the parse, the transition
 * function, and the serializer the router validates against.
 *
 * Nothing outside this module may carry a search across a repoId change: a
 * `?change=` names a path in one repository, and the two places that rewrite a
 * process-local repoId (the route loader's redirect and session restore) are
 * the same repository under a new id.
 */

export type RepoTab = "changes" | "history";
export type RepoView = "overview" | "graph" | "issues" | "pulls";
/** The open/closed slice the issues and pulls lists each show. */
export type ListTab = "open" | "closed";

const REPO_TABS: readonly RepoTab[] = ["changes", "history"];
const REPO_VIEWS: readonly RepoView[] = [
    "overview",
    "graph",
    "issues",
    "pulls",
];
const LIST_TABS: readonly ListTab[] = ["open", "closed"];

export interface RepoState {
    selectedChange: string | null;
    selectedCommit: string | null;
    selectedStash: string | null;
    repoTab: RepoTab;
    view: RepoView;
    issuesTab: ListTab;
    pullsTab: ListTab;
    issueLabel: string;
    pullLabel: string;
}

export type RepoStateAction =
    | { type: "SET_CHANGE"; data: string | null }
    | { type: "SET_COMMIT"; data: string | null }
    | { type: "SET_STASH"; data: string | null }
    | { type: "SET_TAB"; data: RepoTab }
    | { type: "SET_VIEW"; data: RepoView }
    | { type: "SET_ISSUES_TAB"; data: ListTab }
    | { type: "SET_PULLS_TAB"; data: ListTab }
    | { type: "SET_ISSUE_LABEL"; data: string }
    | { type: "SET_PULL_LABEL"; data: string }
    | { type: "CLEAR_SELECTION" };

export type RepoSearch = {
    view?: RepoView;
    tab?: RepoTab;
    change?: string;
    commit?: string;
    stash?: string;
    issuesTab?: ListTab;
    pullsTab?: ListTab;
    issueLabel?: string;
    pullLabel?: string;
};

export const DEFAULT_REPO_STATE: RepoState = {
    selectedChange: null,
    selectedCommit: null,
    selectedStash: null,
    repoTab: "changes",
    view: "overview",
    issuesTab: "open",
    pullsTab: "open",
    issueLabel: "none",
    pullLabel: "none",
};

/** The three selections are mutually exclusive: the main view shows a single
 * diff at a time, and every selection switch drops the one it replaces. */
function clearSelections(state: RepoState): RepoState {
    return {
        ...state,
        selectedChange: null,
        selectedCommit: null,
        selectedStash: null,
    };
}

function currentSelection(state: RepoState): string | null {
    return state.selectedChange ?? state.selectedCommit ?? state.selectedStash;
}

/** A field write that keeps the identity when nothing moved, so a no-op
 * dispatch never rewrites the href. */
function patch<K extends "issuesTab" | "pullsTab" | "issueLabel" | "pullLabel">(
    state: RepoState,
    key: K,
    value: RepoState[K]
): RepoState {
    return state[key] === value ? state : { ...state, [key]: value };
}

/** Pure transition. Returns `state` unchanged when the action is a no-op so
 * callers can skip a navigation. */
export function repoStateReducer(
    state: RepoState,
    action: RepoStateAction
): RepoState {
    switch (action.type) {
        case "SET_CHANGE":
            if (state.selectedChange === action.data) return state;
            return {
                ...state,
                selectedChange: action.data,
                selectedStash: null,
            };
        case "SET_COMMIT":
            if (state.selectedCommit === action.data) return state;
            return {
                ...state,
                selectedCommit: action.data,
                selectedStash: null,
            };
        case "SET_STASH":
            if (state.selectedStash === action.data) return state;
            return {
                ...state,
                selectedStash: action.data,
                selectedChange: null,
                selectedCommit: null,
            };
        case "SET_TAB":
            if (state.repoTab === action.data) return state;
            return { ...clearSelections(state), repoTab: action.data };
        case "SET_VIEW":
            if (state.view === action.data) return state;
            return { ...clearSelections(state), view: action.data };
        // The list slices outlive a view switch: leaving issues for the graph
        // and coming back lands on the same tab and label.
        case "SET_ISSUES_TAB":
            return patch(state, "issuesTab", action.data);
        case "SET_PULLS_TAB":
            return patch(state, "pullsTab", action.data);
        case "SET_ISSUE_LABEL":
            return patch(state, "issueLabel", action.data);
        case "SET_PULL_LABEL":
            return patch(state, "pullLabel", action.data);
        case "CLEAR_SELECTION":
            if (currentSelection(state) === null) return state;
            return clearSelections(state);
    }
}

function text(value: unknown): string | undefined {
    return typeof value === "string" && value.length > 0 ? value : undefined;
}

function oneOf<T extends string>(
    value: unknown,
    allowed: readonly T[]
): T | undefined {
    return allowed.find((candidate) => candidate === value);
}

/** Junk in the query string degrades to the default rather than reaching the
 * page. */
export function parseRepoSearch(search: Record<string, unknown>): RepoState {
    return {
        selectedChange: text(search.change) ?? null,
        selectedCommit: text(search.commit) ?? null,
        selectedStash: text(search.stash) ?? null,
        repoTab: oneOf(search.tab, REPO_TABS) ?? DEFAULT_REPO_STATE.repoTab,
        view: oneOf(search.view, REPO_VIEWS) ?? DEFAULT_REPO_STATE.view,
        issuesTab:
            oneOf(search.issuesTab, LIST_TABS) ?? DEFAULT_REPO_STATE.issuesTab,
        pullsTab:
            oneOf(search.pullsTab, LIST_TABS) ?? DEFAULT_REPO_STATE.pullsTab,
        issueLabel: text(search.issueLabel) ?? DEFAULT_REPO_STATE.issueLabel,
        pullLabel: text(search.pullLabel) ?? DEFAULT_REPO_STATE.pullLabel,
    };
}

/** Defaults are left implicit so a pristine repo page has a clean href. */
export function repoSearchParams(state: RepoState): RepoSearch {
    const params: RepoSearch = {};
    if (state.view !== DEFAULT_REPO_STATE.view) params.view = state.view;
    if (state.repoTab !== DEFAULT_REPO_STATE.repoTab)
        params.tab = state.repoTab;
    if (state.selectedChange !== null) params.change = state.selectedChange;
    if (state.selectedCommit !== null) params.commit = state.selectedCommit;
    if (state.selectedStash !== null) params.stash = state.selectedStash;
    if (state.issuesTab !== DEFAULT_REPO_STATE.issuesTab)
        params.issuesTab = state.issuesTab;
    if (state.pullsTab !== DEFAULT_REPO_STATE.pullsTab)
        params.pullsTab = state.pullsTab;
    if (state.issueLabel !== DEFAULT_REPO_STATE.issueLabel)
        params.issueLabel = state.issueLabel;
    if (state.pullLabel !== DEFAULT_REPO_STATE.pullLabel)
        params.pullLabel = state.pullLabel;
    return params;
}

/** The route's `validateSearch`: unknown keys are dropped, so the canonical
 * search is exactly what `repoSearchParams` can produce and a hand-typed or
 * stale href converges on it. */
export function validateRepoSearch(
    search: Record<string, unknown>
): RepoSearch {
    return repoSearchParams(parseRepoSearch(search));
}
