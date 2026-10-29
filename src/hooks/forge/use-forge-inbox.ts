import {
    useInfiniteQuery,
    useMutation,
    type InfiniteData,
} from "@tanstack/react-query";

import { useAppServices } from "@/contexts/services-context";
import type { NotificationPage } from "@/lib/backend/protocol";
import { infiniteNotificationsQuery } from "@/lib/backend/queries/forge-queries";
import { forgeKeys } from "@/lib/backend/queries/query-keys";
import { expectOk } from "@/lib/backend/transport/result";

import {
    useActiveCapabilities,
    useActiveProvider,
    useForgeAccount,
} from "./use-forge-account";

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
export function useNotifications() {
    const { backend } = useAppServices();
    const provider = useActiveProvider();
    const account = useForgeAccount(provider);
    const capabilities = useActiveCapabilities();
    const login = account.data?.login ?? null;
    // A provider without the inbox capability would only answer with
    // Unsupported, so the read waits for one that serves notifications.
    const query = useInfiniteQuery(
        infiniteNotificationsQuery(
            { backend },
            provider,
            login,
            account.data != null &&
                (capabilities == null || capabilities.notifications)
        )
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
export function useForgeUnreadCount() {
    const { threads, hasNextPage, isLoading } = useNotifications();
    const unread = threads.filter((thread) => thread.unread).length;
    const hasMore = hasNextPage === true;
    return { unread, hasMore, isLoading, exact: !hasMore };
}

export function useMarkNotificationRead() {
    const { backend, queryClient } = useAppServices();
    const provider = useActiveProvider();
    const account = useForgeAccount(provider);
    const login = account.data?.login ?? null;
    const mutation = useMutation({
        mutationFn: async (threadId: string) =>
            expectOk(
                await backend.forge.markNotificationRead(
                    provider ?? "",
                    threadId
                )
            ),
        onSuccess: (_, threadId) => {
            if (provider == null || login == null) return;
            queryClient.setQueryData<InfiniteData<NotificationPage>>(
                forgeKeys.notifications(provider, login),
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
    const provider = useActiveProvider();
    const account = useForgeAccount(provider);
    const login = account.data?.login ?? null;
    const mutation = useMutation({
        mutationFn: async () =>
            expectOk(
                await backend.forge.markAllNotificationsRead(provider ?? "")
            ),
        onSuccess: () => {
            if (provider == null || login == null) return;
            queryClient.setQueryData<InfiniteData<NotificationPage>>(
                forgeKeys.notifications(provider, login),
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
    const provider = useActiveProvider();
    const mutation = useMutation({
        mutationFn: async (subjectUrl: string) =>
            expectOk(
                await backend.forge.resolveSubjectUrl(
                    provider ?? "",
                    subjectUrl
                )
            ),
    });
    return {
        pending: mutation.isPending,
        resolve: (subjectUrl: string) => mutation.mutateAsync(subjectUrl),
    };
}
