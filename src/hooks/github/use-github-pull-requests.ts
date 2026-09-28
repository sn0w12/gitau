import { useInfiniteQuery } from "@tanstack/react-query";

import { useAppServices } from "@/contexts/services-context";
import type { IssueStateFilter } from "@/lib/backend/queries/github-issues-queries";
import { infiniteRepoPullsQuery } from "@/lib/backend/queries/github-pulls-queries";

import { useGithubAccount } from "./use-github-account";
import { useGithubCoords } from "./use-github-issues";

/** Pull requests of the repository open in `repoId`, filtered by state,
 * paginated. */
export function useRepoPullRequests(
    repoId: number | undefined,
    state: IssueStateFilter,
    labels: string[] = []
) {
    const { backend } = useAppServices();
    const account = useGithubAccount();
    const { coords } = useGithubCoords(repoId);
    const query = useInfiniteQuery(
        infiniteRepoPullsQuery(
            { backend },
            coords?.owner ?? null,
            coords?.repo ?? null,
            state,
            labels,
            account.data != null
        )
    );
    const pulls = (query.data?.pages ?? []).flatMap((page) => page.items);
    return { ...query, pulls, coords };
}
