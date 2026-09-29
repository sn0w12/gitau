import { useSearch } from "@tanstack/react-router";

import { useActiveTabRouter } from "@/hooks/tabs/use-active-tab-router";
import {
    type RepoState,
    type RepoStateAction,
    parseRepoSearch,
    repoSearchParams,
    repoStateReducer,
} from "@/lib/routing/repo-search";

/**
 * The repo page's view, panel, and selection read out of the query string and
 * write back through a replace navigation. Nothing is mirrored into component
 * state, so a back navigation, a reload, and a session restore all land on
 * whatever the URL says.
 *
 * `dispatch` keeps the reducer's shape because the components below it already
 * speak in actions; the reducer just computes the next search instead of the
 * next state. The transition runs off the state this render shows, so an
 * action always lands on what the user is looking at.
 */
export function useRepoPageState(
    repoId: number
): [RepoState, (action: RepoStateAction) => void] {
    const search = useSearch({ strict: false });
    const state = parseRepoSearch(search);
    const router = useActiveTabRouter();

    const dispatch = (action: RepoStateAction) => {
        // The reducer hands back the same object for a no-op action, so
        // reselecting the current row never rewrites the href.
        const next = repoStateReducer(state, action);
        if (next === state) return;
        // Replaced, not pushed: the href tracks the page rather than the
        // click-by-click path taken to it, so back leaves the repo page
        // instead of walking back through every row that was selected.
        void router?.navigate({
            to: "/repo/$repoId",
            params: { repoId: String(repoId) },
            search: repoSearchParams(next),
            replace: true,
        });
    };

    return [state, dispatch];
}
