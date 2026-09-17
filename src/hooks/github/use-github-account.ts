import { useMutation, useQuery } from "@tanstack/react-query";
import { useCallback, useState } from "react";

import { useAppServices } from "@/contexts/services-context";
import type { DeviceFlowStart } from "@/lib/backend/protocol";
import { accountQuery, orgsQuery } from "@/lib/backend/queries/github-queries";
import { githubKeys } from "@/lib/backend/queries/query-keys";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import { expectOk } from "@/lib/backend/transport/result";

/** The connected GitHub account, `undefined` while loading, `null` when
 * signed out. */
export function useGithubAccount() {
    const { backend } = useAppServices();
    return useQuery(accountQuery({ backend }));
}

export function useGithubOrgs(enabled: boolean) {
    const { backend } = useAppServices();
    return useQuery(orgsQuery({ backend }, enabled));
}

export interface GithubSignInController {
    /** Present while waiting for browser authorization. */
    start: DeviceFlowStart | null;
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
export function useGithubSignIn(
    onSignedIn?: () => void
): GithubSignInController {
    const { backend, queryClient } = useAppServices();
    const [start, setStart] = useState<DeviceFlowStart | null>(null);

    const mutation = useMutation({
        mutationFn: async () => {
            const started = expectOk(await backend.github.beginSignIn());
            setStart(started);
            try {
                return await expectOk(await backend.github.completeSignIn());
            } finally {
                setStart(null);
            }
        },
        onSuccess: (profile) => {
            queryClient.setQueryData(githubKeys.account(), profile);
            void queryClient.invalidateQueries({
                queryKey: githubKeys.all,
            });
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
        void backend.github.cancelSignIn();
    }, [backend]);

    return {
        start,
        pending: mutation.isPending,
        signIn,
        cancel,
    };
}

export interface GithubSignOutController {
    pending: boolean;
    signOut: () => Promise<void>;
}

export function useGithubSignOut(): GithubSignOutController {
    const { backend, queryClient } = useAppServices();
    const mutation = useMutation({
        mutationFn: async () => expectOk(await backend.github.signOut()),
        onSuccess: () => {
            queryClient.setQueryData(githubKeys.account(), null);
            void queryClient.invalidateQueries({ queryKey: githubKeys.all });
        },
    });

    return {
        pending: mutation.isPending,
        signOut: async () => {
            await mutation.mutateAsync().catch(() => {});
        },
    };
}
