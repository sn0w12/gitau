import { useMutation, useQueries, useQuery } from "@tanstack/react-query";
import { useSelector } from "@tanstack/react-store";
import { useCallback, useState } from "react";

import { useAppServices } from "@/contexts/services-context";
import type {
    AuthChallenge,
    Capabilities,
    ConnectionProfile,
} from "@/lib/backend/protocol";
import {
    accountQuery,
    connectionsQuery,
    orgsQuery,
    providersQuery,
} from "@/lib/backend/queries/forge-queries";
import { forgeKeys } from "@/lib/backend/queries/query-keys";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import { expectOk } from "@/lib/backend/transport/result";
import { connectionKey, connectionStore } from "@/stores/connection-store";

/** Providers this build can talk to, with their capabilities. */
export function useForgeProviders() {
    const { backend } = useAppServices();
    return useQuery(providersQuery({ backend }));
}

/** Every connection that currently holds a token. */
export function useForgeConnections() {
    const { backend } = useAppServices();
    return useQuery(connectionsQuery({ backend }));
}

/** The connected account on one provider, `undefined` while loading, `null`
 * when signed out. */
export function useForgeAccount(provider: string | null) {
    const { backend } = useAppServices();
    return useQuery({
        ...accountQuery({ backend }, provider ?? ""),
        enabled: provider != null,
    });
}

/**
 * The connection the account-scoped pages scope to. Follows the explicit
 * selection while it still points at a live connection, so signing out or
 * removing the selected one falls back to the first instead of going blank.
 */
export function useActiveConnection(): ConnectionProfile | null {
    const connections = useForgeConnections();
    const activeKey = useSelector(connectionStore, (state) => state.activeKey);
    const list = connections.data;
    if (!list || list.length === 0) return null;
    return (
        list.find(
            (entry) => connectionKey(entry.provider, entry.host) === activeKey
        ) ?? list[0]
    );
}

/** The provider the account-scoped pages scope to, falling back to the
 * registry's default while nothing is connected. */
export function useActiveProvider(): string | null {
    const connection = useActiveConnection();
    const providers = useForgeProviders();
    return connection?.provider ?? providers.data?.[0]?.id ?? null;
}

/** What the active provider can do, so views and nav entries that need a
 * capability render disabled instead of dead. */
export function useActiveCapabilities(): Capabilities | null {
    const provider = useActiveProvider();
    const providers = useForgeProviders();
    if (provider == null) return null;
    return (
        providers.data?.find((entry) => entry.id === provider)?.capabilities ??
        null
    );
}

/**
 * Whether a capability-gated control should be offered. Unknown capabilities
 * (still loading, or no provider to ask) read as available, so nothing
 * flickers disabled while the provider list loads.
 */
export function capabilityAllows(
    capabilities: Capabilities | null,
    flag: keyof Capabilities
): boolean {
    return capabilities == null || capabilities[flag];
}

export interface PublishOwnerEntry {
    /** Selection value: `provider/owner`, with an empty owner for the
     * personal account. */
    value: string;
    label: string;
    /** The organization login, or null for the personal account. */
    owner: string | null;
}

export interface PublishOwnerGroup {
    provider: string;
    label: string;
    entries: PublishOwnerEntry[];
}

/**
 * Every owner a repository can be published under, grouped by provider, so
 * one control picks the provider and the owner together. Orgs are read only
 * for a provider the account is signed in to, since the list needs a token.
 */
export function usePublishOwnerGroups(): {
    groups: PublishOwnerGroup[];
    hasPublishProvider: boolean;
} {
    const { backend } = useAppServices();
    const providers = useForgeProviders();
    const publishable = (providers.data ?? []).filter(
        (entry) => entry.capabilities.publish
    );
    const accounts = useQueries({
        queries: publishable.map((entry) =>
            accountQuery({ backend }, entry.id)
        ),
    });
    const orgs = useQueries({
        queries: publishable.map((entry, index) => ({
            ...orgsQuery({ backend }, entry.id, true),
            enabled: accounts[index]?.data != null,
        })),
    });

    const groups = publishable.map((entry, index) => {
        const account = accounts[index]?.data ?? null;
        const entries: PublishOwnerEntry[] = [];
        if (account) {
            entries.push({
                value: `${entry.id}/`,
                label: `${account.login} (personal)`,
                owner: null,
            });
        }
        for (const org of orgs[index]?.data ?? []) {
            entries.push({
                value: `${entry.id}/${org.login}`,
                label: org.login,
                owner: org.login,
            });
        }
        return { provider: entry.id, label: entry.label, entries };
    });

    return { groups, hasPublishProvider: publishable.length > 0 };
}

export interface ForgeSignInController {
    /** Present while waiting for browser authorization. */
    start: AuthChallenge | null;
    pending: boolean;
    /**
     * Runs the device flow to completion. Resolves quietly when the user
     * cancels; rejects with the failure otherwise so the caller can toast.
     */
    signIn: () => Promise<void>;
    cancel: () => void;
}

/**
 * Drives the OAuth device flow: begin returns the short code shown in the
 * UI, then complete waits until the user finishes browser authorization,
 * the code expires, or cancel is requested.
 */
export function useForgeSignIn(
    provider: string,
    host: string,
    onSignedIn?: () => void
): ForgeSignInController {
    const { backend, queryClient } = useAppServices();
    const [start, setStart] = useState<AuthChallenge | null>(null);

    const mutation = useMutation({
        mutationFn: async () => {
            const started = expectOk(
                await backend.forge.beginSignIn(provider, host)
            );
            setStart(started);
            try {
                return await expectOk(await backend.forge.completeSignIn());
            } finally {
                setStart(null);
            }
        },
        onSuccess: () => {
            void queryClient.invalidateQueries({ queryKey: forgeKeys.all });
            onSignedIn?.();
        },
    });

    const signIn = useCallback(async () => {
        try {
            await mutation.mutateAsync();
        } catch (error) {
            if (error instanceof GitBackendError && error.isCancelled) return;
            throw error;
        }
    }, [mutation]);

    const cancel = useCallback(() => {
        void backend.forge.cancelSignIn();
    }, [backend]);

    return {
        start,
        pending: mutation.isPending,
        signIn,
        cancel,
    };
}

export interface ForgeSignOutController {
    pending: boolean;
    signOut: () => Promise<void>;
}

export function useForgeSignOut(
    provider: string,
    host: string
): ForgeSignOutController {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async () =>
            expectOk(await backend.forge.signOut(provider, host)),
        onSuccess: () => {
            void queryClient.invalidateQueries({ queryKey: forgeKeys.all });
        },
    });

    return {
        pending: mutation.isPending,
        signOut: async () => {
            await mutation.mutateAsync().catch(() => {});
        },
    };
}
