import { ExternalLinkIcon } from "lucide-react";
import { useState } from "react";

import { ExternalLink } from "@/components/external-link";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import {
    Frame,
    FrameDescription,
    FrameHeader,
    FramePanel,
    FrameTitle,
} from "@/components/ui/frame";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
    Select,
    SelectItem,
    SelectPopup,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { useConfirm } from "@/contexts/confirm-context";
import {
    useForgeAccount,
    useForgeProviders,
    useForgeSignIn,
    useForgeSignOut,
} from "@/hooks/forge/use-forge-account";
import { useRemoteIcon } from "@/hooks/repositories/use-repository-queries";
import type { ProviderDescriptor } from "@/lib/backend/protocol";
import { toastError } from "@/lib/toast-error";

/**
 * Connection management for every provider this build supports: device-flow
 * sign-in per host, profile summary, and sign-out.
 */
export function AccountPage() {
    const providers = useForgeProviders();

    return (
        <div className="container h-full min-h-0 p-2">
            <ScrollArea className="h-full" scrollFade>
                <div className="mx-auto flex max-w-2xl flex-col gap-1 p-1">
                    {providers.isLoading ? (
                        <Frame className="gap-1 p-1">
                            <FramePanel className="flex items-center justify-center gap-2 py-8 text-sm text-muted-foreground">
                                <Spinner className="size-4" />
                                Checking connections...
                            </FramePanel>
                        </Frame>
                    ) : (providers.data ?? []).length === 0 ? (
                        <Frame className="gap-1 p-1">
                            <FramePanel className="py-8 text-center text-sm text-muted-foreground">
                                This build has no providers.
                            </FramePanel>
                        </Frame>
                    ) : (
                        (providers.data ?? []).map((provider) => (
                            <ProviderCard
                                key={provider.id}
                                provider={provider}
                            />
                        ))
                    )}
                </div>
            </ScrollArea>
        </div>
    );
}

function ProviderCard({ provider }: { provider: ProviderDescriptor }) {
    const [host, setHost] = useState<string>(provider.hosts[0] ?? "");
    const account = useForgeAccount(provider.id);

    const description = `Connect ${provider.label} to work with private repositories and publish local projects.`;

    return (
        <Frame
            className="gap-1 p-1"
            data-testid={`provider-card-${provider.id}`}
        >
            <FrameHeader className="px-3 py-1.5">
                <FrameTitle className="text-base font-semibold">
                    {provider.label}
                </FrameTitle>
                <FrameDescription className="text-sm text-muted-foreground">
                    {description}
                </FrameDescription>
            </FrameHeader>
            <FramePanel data-testid="account-panel" className="p-2">
                {account.isLoading ? (
                    <div className="flex items-center justify-center gap-2 py-8 text-sm text-muted-foreground">
                        <Spinner className="size-4" />
                        Checking connection...
                    </div>
                ) : account.data ? (
                    <ConnectedCard provider={provider} host={host} />
                ) : (
                    <SignInCard
                        provider={provider}
                        host={host}
                        onHostChange={setHost}
                    />
                )}
            </FramePanel>
        </Frame>
    );
}

function HostSelect({
    provider,
    host,
    onHostChange,
}: {
    provider: ProviderDescriptor;
    host: string;
    onHostChange: (host: string) => void;
}) {
    if (provider.hosts.length <= 1) return null;
    return (
        <Field className="w-full max-w-xs">
            <FieldLabel>Host</FieldLabel>
            <Select
                value={host}
                onValueChange={(value) => {
                    if (typeof value === "string") onHostChange(value);
                }}
                items={provider.hosts.map((entry) => ({
                    label: entry,
                    value: entry,
                }))}
            >
                <SelectTrigger className="w-full">
                    <SelectValue />
                </SelectTrigger>
                <SelectPopup>
                    {provider.hosts.map((entry) => (
                        <SelectItem key={entry} value={entry}>
                            {entry}
                        </SelectItem>
                    ))}
                </SelectPopup>
            </Select>
        </Field>
    );
}

function SignInCard({
    provider,
    host,
    onHostChange,
}: {
    provider: ProviderDescriptor;
    host: string;
    onHostChange: (host: string) => void;
}) {
    const [signInError, setSignInError] = useState<string | null>(null);
    const signIn = useForgeSignIn(provider.id, host, () =>
        setSignInError(null)
    );

    const handleSignIn = async () => {
        setSignInError(null);
        try {
            await signIn.signIn();
        } catch (error) {
            toastError(`${provider.label} sign-in failed`, error);
        }
    };

    if (signIn.start) {
        return (
            <div className="flex flex-col items-center gap-2 py-2">
                <div className="text-center">
                    <p className="font-medium">
                        Enter this code on {provider.label}
                    </p>
                    <p className="text-sm text-muted-foreground">
                        {provider.label} asks for it at{" "}
                        <span className="font-mono">{host}/login/device</span>
                    </p>
                </div>
                <code
                    data-testid="forge-user-code"
                    className="rounded-lg border bg-muted px-6 py-3 font-mono text-3xl font-semibold tracking-[0.3em] select-all"
                >
                    {signIn.start.userCode}
                </code>
                <div className="flex items-center gap-2">
                    <a
                        href={signIn.start.verificationUri}
                        target="_blank"
                        rel="noreferrer"
                    >
                        <Button>
                            Open {host}/login/device
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
                        data-testid="forge-cancel-signin"
                        onClick={signIn.cancel}
                    >
                        Cancel
                    </Button>
                </div>
            </div>
        );
    }

    return (
        <div className="flex flex-col items-center gap-5 py-2">
            {signInError ? (
                <p className="text-sm text-destructive" role="alert">
                    {signInError}
                </p>
            ) : null}
            <HostSelect
                provider={provider}
                host={host}
                onHostChange={onHostChange}
            />
            <Button
                size="lg"
                disabled={signIn.pending}
                loading={signIn.pending}
                onClick={() => void handleSignIn()}
            >
                Sign in with {provider.label}
            </Button>
        </div>
    );
}

function ConnectedCard({
    provider,
    host,
}: {
    provider: ProviderDescriptor;
    host: string;
}) {
    const account = useForgeAccount(provider.id);
    const profile = account.data;
    const { confirm } = useConfirm();
    const signOut = useForgeSignOut(provider.id, host);

    // Resolve through the shared icon pipeline so the image is cached as a
    // data URL instead of loaded from the network inside the webview.
    const avatar = useRemoteIcon(
        profile ? `https://${host}/${profile.login}` : undefined
    );

    if (!profile) return null;

    const handleSignOut = async () => {
        const result = await confirm({
            title: `Sign out of ${provider.label}?`,
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
            data-testid="forge-account-connected"
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
                    {host} · connected{" "}
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
