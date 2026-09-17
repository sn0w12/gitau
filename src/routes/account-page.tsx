"use no memo";

import { ExternalLinkIcon } from "lucide-react";
import { useState } from "react";

import { ExternalLink } from "@/components/external-link";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import {
    Frame,
    FrameDescription,
    FrameHeader,
    FramePanel,
    FrameTitle,
} from "@/components/ui/frame";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Spinner } from "@/components/ui/spinner";
import { useConfirm } from "@/contexts/confirm-context";
import {
    useGithubAccount,
    useGithubSignIn,
    useGithubSignOut,
} from "@/hooks/github/use-github-account";
import { useRemoteIcon } from "@/hooks/repositories/use-repository-queries";
import { toastError } from "@/lib/toast-error";

/**
 * GitHub connection management: device-flow sign-in, profile summary, and
 * sign-out.
 */
export function AccountPage() {
    const account = useGithubAccount();
    const [signInError, setSignInError] = useState<string | null>(null);

    const signInController = useGithubSignIn(() => setSignInError(null));

    const handleSignIn = async () => {
        setSignInError(null);
        try {
            await signInController.signIn();
        } catch (error) {
            toastError("GitHub sign-in failed", error);
        }
    };

    return (
        <div className="container h-full min-h-0 p-2">
            <ScrollArea className="h-full" scrollFade>
                <Frame className="mx-auto max-w-2xl gap-1 p-1">
                    <FrameHeader className="px-3 py-1.5">
                        <FrameTitle className="text-base font-semibold">
                            GitHub
                        </FrameTitle>
                        <FrameDescription className="text-sm text-muted-foreground">
                            Connect a GitHub account to work with private
                            repositories and publish local projects.
                        </FrameDescription>
                    </FrameHeader>
                    <FramePanel data-testid="account-panel" className="p-2">
                        {account.isLoading ? (
                            <div className="flex items-center justify-center gap-2 py-8 text-sm text-muted-foreground">
                                <Spinner className="size-4" />
                                Checking connection...
                            </div>
                        ) : account.data ? (
                            <ConnectedCard />
                        ) : (
                            <SignInCard
                                pending={signInController.pending}
                                start={signInController.start}
                                error={signInError}
                                onSignIn={() => void handleSignIn()}
                                onCancel={signInController.cancel}
                            />
                        )}
                    </FramePanel>
                </Frame>
            </ScrollArea>
        </div>
    );
}

function SignInCard({
    pending,
    start,
    error,
    onSignIn,
    onCancel,
}: {
    pending: boolean;
    start: ReturnType<typeof useGithubSignIn>["start"];
    error: string | null;
    onSignIn: () => void;
    onCancel: () => void;
}) {
    if (start) {
        return (
            <div className="flex flex-col items-center gap-2 py-2">
                <div className="text-center">
                    <p className="font-medium">Enter this code on GitHub</p>
                    <p className="text-sm text-muted-foreground">
                        GitHub asks for it at{" "}
                        <span className="font-mono">
                            github.com/login/device
                        </span>
                    </p>
                </div>
                <code
                    data-testid="github-user-code"
                    className="rounded-lg border bg-muted px-6 py-3 font-mono text-3xl font-semibold tracking-[0.3em] select-all"
                >
                    {start.userCode}
                </code>
                <div className="flex items-center gap-2">
                    <a
                        href={start.verificationUri}
                        target="_blank"
                        rel="noreferrer"
                    >
                        <Button>
                            Open github.com/login/device
                            <ExternalLinkIcon />
                        </Button>
                    </a>
                </div>
                <div className="flex items-center gap-2 text-sm text-muted-foreground">
                    <Spinner className="size-3.5" />
                    Waiting for authorization...
                    <Button
                        variant="ghost"
                        size="sm"
                        data-testid="github-cancel-signin"
                        onClick={onCancel}
                    >
                        Cancel
                    </Button>
                </div>
            </div>
        );
    }

    return (
        <div className="flex flex-col items-center gap-5 py-2">
            {error ? (
                <p className="text-sm text-destructive" role="alert">
                    {error}
                </p>
            ) : null}
            <Button
                size="lg"
                disabled={pending}
                loading={pending}
                onClick={onSignIn}
            >
                Sign in with GitHub
            </Button>
        </div>
    );
}

function ConnectedCard() {
    const account = useGithubAccount();
    const profile = account.data;
    const { confirm } = useConfirm();
    const signOut = useGithubSignOut();

    // Resolve through the shared icon pipeline so the image is cached as a
    // data URL instead of loaded from the network inside the webview.
    const avatar = useRemoteIcon(
        profile ? `https://github.com/${profile.login}` : undefined
    );

    if (!profile) return null;

    const handleSignOut = async () => {
        const result = await confirm({
            title: "Sign out of GitHub?",
            description:
                "gitau removes its stored token from your system keychain. Private repositories will require signing in again.",
            confirmText: "Sign out",
            variant: "destructive",
        });
        if (!result.confirmed) return;
        try {
            await signOut.signOut();
        } catch (error) {
            toastError("Could not sign out", error);
        }
    };

    return (
        <div
            className="flex items-center gap-2"
            data-testid="github-account-connected"
        >
            <Avatar className="size-8">
                <AvatarImage
                    src={avatar.data?.dataUrl}
                    alt={`${profile.login} avatar`}
                />
                <AvatarFallback>
                    {(profile.login.charAt(0) || "?").toUpperCase()}
                </AvatarFallback>
            </Avatar>
            <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                    <ExternalLink
                        href={profile.htmlUrl || undefined}
                        className="truncate font-semibold hover:underline"
                    >
                        {profile.name ?? profile.login}
                    </ExternalLink>
                    {profile.name ? (
                        <span className="truncate font-mono text-sm text-muted-foreground">
                            {profile.login}
                        </span>
                    ) : null}
                </div>
                <p className="text-xs text-muted-foreground">
                    Connected{" "}
                    {new Date(profile.connectedAtMs).toLocaleDateString()}
                </p>
            </div>
            <Button
                variant="destructive-outline"
                disabled={signOut.pending}
                loading={signOut.pending}
                onClick={() => void handleSignOut()}
            >
                Sign out
            </Button>
        </div>
    );
}
