import { useBackgroundFetch } from "@/hooks/repositories/use-background-fetch";

/**
 * App-wide null component that drives GitHub Desktop-style background
 * fetching for every open repository.
 */
export function BackgroundFetch() {
    useBackgroundFetch();
    return null;
}
