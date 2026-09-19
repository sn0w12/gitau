import {
    useInfiniteQuery,
    useMutation,
    type InfiniteData,
} from "@tanstack/react-query";

import { useAppServices } from "@/contexts/services-context";
import type { NotificationPage } from "@/lib/backend/protocol";
import { infiniteNotificationsQuery } from "@/lib/backend/queries/github-queries";
import { githubKeys } from "@/lib/backend/queries/query-keys";
import { expectOk } from "@/lib/backend/transport/result";

import { useGithubAccount } from "./use-github-account";

function updateThread(
    data: InfiniteData<NotificationPage>,
    threadId: string
): InfiniteData<NotificationPage> {
    return {
        ...data,
        pages: data.pages.map((page) => ({
            ...page,
            notifications: page.notifications.map((thread) =>
                thread.id === threadId ? { ...thread, unread: false } : thread
            ),
        })),
    };
}

function updateAllRead(
    data: InfiniteData<NotificationPage>
): InfiniteData<NotificationPage> {
    return {
        ...data,
        pages: data.pages.map((page) => ({
            ...page,
            notifications: page.notifications.map((thread) =>
                thread.unread ? { ...thread, unread: false } : thread
            ),
        })),
    };
}

/**
 * Paginated inbox, newest first. Shares one cache entry with the unread
 * badge, so the sidebar and the page never fetch twice. Pages after the
 * first load only via `fetchNextPage`.
 */
export function useGithubNotifications() {
    const { backend } = useAppServices();
    const account = useGithubAccount();
    const query = useInfiniteQuery(
        infiniteNotificationsQuery({ backend }, account.data != null)
    );
    const threads = (query.data?.pages ?? []).flatMap(
        (page) => page.notifications
    );
    return { ...query, threads };
}

/**
 * Unread count across loaded pages. While later pages are still unfetched
 * the count is a lower bound, so `exact` is false; once all pages are
 * fetched the count is exact.
 */
export function useGithubUnreadCount() {
    const { threads, hasNextPage, isLoading } = useGithubNotifications();
    const unread = threads.filter((thread) => thread.unread).length;
    const hasMore = hasNextPage === true;
    return { unread, hasMore, isLoading, exact: !hasMore };
}

export function useMarkNotificationRead() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (threadId: string) =>
            expectOk(await backend.github.markNotificationRead(threadId)),
        onSuccess: (_, threadId) => {
            queryClient.setQueryData<InfiniteData<NotificationPage>>(
                githubKeys.notifications(),
                (data) => (data ? updateThread(data, threadId) : data)
            );
        },
    });
    return {
        pending: mutation.isPending,
        markRead: (threadId: string) => mutation.mutateAsync(threadId),
    };
}

export function useMarkAllNotificationsRead() {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async () =>
            expectOk(await backend.github.markAllNotificationsRead()),
        onSuccess: () => {
            queryClient.setQueryData<InfiniteData<NotificationPage>>(
                githubKeys.notifications(),
                (data) => (data ? updateAllRead(data) : data)
            );
        },
    });
    return {
        pending: mutation.isPending,
        markAllRead: () => mutation.mutateAsync(),
    };
}

/**
 * Resolves a subject API URL to its web URL, for notification types
 * without a static mapping. Resolves `null` when the subject is gone.
 */
export function useResolveSubjectUrl() {
    const { backend } = useAppServices();
    const mutation = useMutation({
        mutationFn: async (subjectUrl: string) =>
            expectOk(await backend.github.resolveSubjectUrl(subjectUrl)),
    });
    return {
        pending: mutation.isPending,
        resolve: (subjectUrl: string) => mutation.mutateAsync(subjectUrl),
    };
}
