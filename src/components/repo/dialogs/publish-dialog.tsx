import { useState } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
    Dialog,
    DialogClose,
    DialogDescription,
    DialogFooter,
    DialogHeader,
    DialogPanel,
    DialogPopup,
    DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
    Select,
    SelectItem,
    SelectPopup,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import {
    useGithubAccount,
    useGithubOrgs,
} from "@/hooks/github/use-github-account";
import { repoDisplayName } from "@/hooks/repositories/use-repo-identity";
import { usePublishToGitHubMutation } from "@/lib/backend/mutations/repository-mutations";
import type { PublishResult } from "@/lib/backend/protocol";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import { toastError } from "@/lib/toast-error";

const PERSONAL_OWNER = "";

/**
 * Creates a GitHub repository for the current local repository: owner
 * (personal or org), name, description, visibility. On success the backend
 * has already wired up `origin` and pushed the current branch.
 *
 * Always mounted and driven by `open`; the form and its queries live inside
 * the portal body, so they mount fresh on every open.
 */
export function PublishToGitHubDialog({
    repoId,
    repoPath,
    open,
    onClose,
}: {
    repoId: number;
    repoPath: string;
    open: boolean;
    onClose: () => void;
}) {
    return (
        <Dialog
            open={open}
            onOpenChange={(next) => {
                if (!next) onClose();
            }}
        >
            <DialogPopup data-testid="publish-dialog">
                <PublishToGitHubDialogBody
                    repoId={repoId}
                    repoPath={repoPath}
                    onClose={onClose}
                />
            </DialogPopup>
        </Dialog>
    );
}

function PublishToGitHubDialogBody({
    repoId,
    repoPath,
    onClose,
}: {
    repoId: number;
    repoPath: string;
    onClose: () => void;
}) {
    const account = useGithubAccount();
    const signedIn = !!account.data;

    const [owner, setOwner] = useState<string>(PERSONAL_OWNER);
    const [name, setName] = useState<string>(() => repoDisplayName(repoPath));
    const [description, setDescription] = useState("");
    const [isPrivate, setIsPrivate] = useState(true);
    const [conflict, setConflict] = useState<string | null>(null);

    const orgs = useGithubOrgs(signedIn);

    const mutation = usePublishToGitHubMutation(repoId);

    const handleSubmit = async () => {
        setConflict(null);
        try {
            const result: PublishResult = await mutation.mutateAsync({
                owner: owner === PERSONAL_OWNER ? undefined : owner,
                name,
                description: description || undefined,
                private: isPrivate,
            });
            onClose();
            return result;
        } catch (error) {
            if (isNameTaken(error)) {
                setConflict(
                    `The name "${name}" is already taken${
                        owner ? ` under ${owner}` : ""
                    }. Pick another one.`
                );
                return undefined;
            }
            toastError("Publishing failed", error);
            return undefined;
        }
    };

    const canSubmit = signedIn && name.trim().length > 0 && !mutation.isPending;

    const personalLabel = account.data?.login
        ? `${account.data.login} (personal)`
        : "Personal";
    const ownerItems = [
        { label: personalLabel, value: PERSONAL_OWNER },
        ...(orgs.data ?? []).map((org) => ({
            label: org.login,
            value: org.login,
        })),
    ];

    return (
        <>
            <DialogHeader>
                <DialogTitle>Publish to GitHub</DialogTitle>
                <DialogDescription>
                    Creates the repository on GitHub, points origin at it, and
                    pushes your current branch.
                </DialogDescription>
            </DialogHeader>
            <DialogPanel className="flex flex-col gap-3">
                {!signedIn ? (
                    <p className="text-sm text-muted-foreground">
                        Connect a GitHub account first: open Account from the
                        sidebar and sign in.
                    </p>
                ) : (
                    <>
                        <div className="flex flex-col gap-1.5">
                            <Label htmlFor="publish-owner">Owner</Label>
                            <Select
                                value={owner}
                                onValueChange={(value) => {
                                    if (typeof value === "string")
                                        setOwner(value);
                                }}
                                items={ownerItems}
                            >
                                <SelectTrigger
                                    className="w-full"
                                    aria-label="Owner"
                                >
                                    <SelectValue />
                                </SelectTrigger>
                                <SelectPopup>
                                    {ownerItems.map(({ label, value }) => (
                                        <SelectItem key={value} value={value}>
                                            {label}
                                        </SelectItem>
                                    ))}
                                </SelectPopup>
                            </Select>
                        </div>
                        <div className="flex flex-col gap-1.5">
                            <Label htmlFor="publish-name">Name</Label>
                            <Input
                                id="publish-name"
                                value={name}
                                onChange={(event) =>
                                    setName(event.target.value)
                                }
                                placeholder="my-project"
                                autoFocus
                            />
                            {conflict ? (
                                <p
                                    className="text-sm text-destructive"
                                    role="alert"
                                >
                                    {conflict}
                                </p>
                            ) : null}
                        </div>
                        <div className="flex flex-col gap-1.5">
                            <Label htmlFor="publish-description">
                                Description
                            </Label>
                            <Input
                                id="publish-description"
                                value={description}
                                onChange={(event) =>
                                    setDescription(event.target.value)
                                }
                                placeholder="Optional"
                            />
                        </div>
                        <div className="flex items-center justify-between">
                            <div className="flex items-center gap-2">
                                <Switch
                                    id="publish-private"
                                    checked={isPrivate}
                                    onCheckedChange={(checked) =>
                                        setIsPrivate(checked === true)
                                    }
                                />
                                <Label htmlFor="publish-private">Private</Label>
                            </div>
                            <Badge variant="secondary">
                                {isPrivate ? "private" : "public"}
                            </Badge>
                        </div>
                    </>
                )}
            </DialogPanel>
            <DialogFooter>
                <DialogClose render={<Button variant="ghost" />}>
                    Cancel
                </DialogClose>
                <Button
                    disabled={!canSubmit}
                    loading={mutation.isPending}
                    onClick={() => void handleSubmit()}
                >
                    Create repository
                </Button>
            </DialogFooter>
        </>
    );
}

function isNameTaken(error: unknown): boolean {
    if (!(error instanceof GitBackendError)) return false;
    return (
        error.code === "github" &&
        /already exist/i.test(error.detail ?? error.message)
    );
}
