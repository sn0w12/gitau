import { useState } from "react";

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
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
import { Field, FieldLabel } from "@/components/ui/field";
import { Form } from "@/components/ui/form";
import { Input } from "@/components/ui/input";
import {
    Select,
    SelectItem,
    SelectPopup,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import {
    useForgeAccount,
    useForgeProviders,
} from "@/hooks/forge/use-forge-account";
import {
    useRepoCapabilities,
    useRepoCoords,
} from "@/hooks/forge/use-forge-issues";
import { useCreatePullRequest } from "@/hooks/forge/use-forge-pull-requests";
import {
    useRepoListing,
    useRepositorySnapshot,
} from "@/hooks/repositories/use-repository-queries";
import { useActiveTabRouter } from "@/hooks/tabs/use-active-tab-router";
import { usePushMutation } from "@/lib/backend/mutations/repository-mutations";
import { pickRepoRemote } from "@/lib/forge/repo-coords";
import { pickBaseBranch } from "@/lib/repositories/default-branch";
import { toastError } from "@/lib/toast-error";

/**
 * Opens a pull request from a branch into another one. Unpushed commits are
 * pushed first. Always mounted and driven by `open`; the form lives inside the
 * portal body, so it remounts fresh on every open.
 */
export function CreatePullDialog({
    repoId,
    initialHead,
    open,
    onClose,
}: {
    repoId: number;
    /** Branch preselected as the head, e.g. the row that opened the dialog. */
    initialHead?: string;
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
            <DialogPopup data-testid="create-pull-dialog">
                <CreatePullForm
                    repoId={repoId}
                    initialHead={initialHead}
                    onClose={onClose}
                />
            </DialogPopup>
        </Dialog>
    );
}

function CreatePullForm({
    repoId,
    initialHead,
    onClose,
}: {
    repoId: number;
    initialHead?: string;
    onClose: () => void;
}) {
    const { coords, data: remotes } = useRepoCoords(repoId);
    const providers = useForgeProviders();
    const capabilities = useRepoCapabilities(repoId);
    const account = useForgeAccount(coords?.provider ?? null);
    const listing = useRepoListing(repoId);
    const snapshot = useRepositorySnapshot(repoId);
    const push = usePushMutation(repoId);
    const create = useCreatePullRequest();
    const router = useActiveTabRouter();

    const branches = listing.data?.branches ?? [];
    const names = branches.map((branch) => branch.name);
    const currentName =
        snapshot.data?.head.state === "attached"
            ? snapshot.data.head.branch
            : null;

    const [head, setHead] = useState<string | null>(null);
    const [base, setBase] = useState<string | null>(null);
    const [title, setTitle] = useState<string | null>(null);
    const [description, setDescription] = useState("");
    const [draft, setDraft] = useState(false);

    const headValue =
        head ??
        (initialHead != null && names.includes(initialHead)
            ? initialHead
            : null) ??
        currentName ??
        names[0] ??
        "";
    const baseValue = base ?? pickBaseBranch(names, headValue);
    const titleValue = title ?? headValue;

    const headBranch = branches.find((branch) => branch.name === headValue);
    const ahead = headBranch?.upstream?.ahead ?? 0;
    const needsPush =
        headBranch != null && (headBranch.upstream == null || ahead > 0);
    const pushRemote =
        pickRepoRemote(remotes ?? [], providers.data ?? [])?.name ?? "origin";

    const providerLabel =
        providers.data?.find((entry) => entry.id === coords?.provider)?.label ??
        coords?.provider ??
        "the provider";
    const unsupported = capabilities != null && !capabilities.pullRequestsWrite;

    const notice =
        coords == null
            ? "No remote of this repository points at a provider this build knows, so there is nowhere to open a pull request."
            : unsupported
              ? `${providerLabel} cannot open pull requests from this build.`
              : account.data == null
                ? `Connect a ${providerLabel} account first: open Account from the sidebar and sign in.`
                : names.length < 2
                  ? "This repository needs a second branch to merge into."
                  : null;

    const canSubmit =
        notice == null &&
        headValue !== "" &&
        baseValue !== "" &&
        baseValue !== headValue &&
        titleValue.trim().length > 0 &&
        !push.isPending &&
        !create.pending;

    const handleSubmit = async () => {
        if (!canSubmit || coords == null) return;
        try {
            if (needsPush) {
                await push.mutateAsync({
                    remote: pushRemote,
                    refspecs: [headValue],
                    setUpstream: true,
                });
            }
            const pull = await create.createPull(
                coords.provider,
                coords.owner,
                coords.repo,
                {
                    title: titleValue.trim(),
                    head: headValue,
                    base: baseValue,
                    body: description.trim() || undefined,
                    draft,
                }
            );
            onClose();
            await router?.navigate({
                to: `/repo/${repoId}/pull/${pull.number}`,
            });
        } catch (error) {
            toastError("Could not create pull request", error);
        }
    };

    const branchItems = (values: string[]) =>
        values.map((name) => ({ value: name, label: name }));

    return (
        <>
            <DialogHeader>
                <DialogTitle>New pull request</DialogTitle>
                <DialogDescription>
                    {coords == null
                        ? "Pull requests need a repository on GitHub."
                        : `Opens a pull request on ${coords.owner}/${coords.repo}.`}
                </DialogDescription>
            </DialogHeader>
            <Form
                className="contents"
                onSubmit={(event) => {
                    event.preventDefault();
                    void handleSubmit();
                }}
            >
                <DialogPanel className="flex flex-col gap-3">
                    {notice == null ? (
                        <>
                            <div className="grid grid-cols-2 gap-3">
                                <Field>
                                    <FieldLabel>Head</FieldLabel>
                                    <Select
                                        value={headValue}
                                        onValueChange={(next) => {
                                            if (typeof next !== "string")
                                                return;
                                            setHead(next);
                                            if (baseValue === next)
                                                setBase(null);
                                        }}
                                        items={branchItems(names)}
                                    >
                                        <SelectTrigger className="w-full">
                                            <SelectValue />
                                        </SelectTrigger>
                                        <SelectPopup>
                                            {names.map((name) => (
                                                <SelectItem
                                                    key={name}
                                                    value={name}
                                                >
                                                    {name}
                                                </SelectItem>
                                            ))}
                                        </SelectPopup>
                                    </Select>
                                </Field>
                                <Field>
                                    <FieldLabel>Base</FieldLabel>
                                    <Select
                                        value={baseValue}
                                        onValueChange={(next) => {
                                            if (typeof next === "string")
                                                setBase(next);
                                        }}
                                        items={branchItems(
                                            names.filter(
                                                (name) => name !== headValue
                                            )
                                        )}
                                    >
                                        <SelectTrigger className="w-full">
                                            <SelectValue />
                                        </SelectTrigger>
                                        <SelectPopup>
                                            {names
                                                .filter(
                                                    (name) => name !== headValue
                                                )
                                                .map((name) => (
                                                    <SelectItem
                                                        key={name}
                                                        value={name}
                                                    >
                                                        {name}
                                                    </SelectItem>
                                                ))}
                                        </SelectPopup>
                                    </Select>
                                </Field>
                            </div>
                            <Field>
                                <FieldLabel>Title</FieldLabel>
                                <Input
                                    value={titleValue}
                                    onChange={(event) =>
                                        setTitle(event.target.value)
                                    }
                                    autoFocus
                                />
                            </Field>
                            <Field>
                                <FieldLabel>Description</FieldLabel>
                                <Textarea
                                    value={description}
                                    onChange={(event) =>
                                        setDescription(event.target.value)
                                    }
                                    rows={4}
                                    placeholder="Optional"
                                />
                            </Field>
                            <Field>
                                <FieldLabel>
                                    <Checkbox
                                        checked={draft}
                                        onCheckedChange={(checked) =>
                                            setDraft(checked === true)
                                        }
                                    />
                                    Draft
                                </FieldLabel>
                            </Field>
                            {needsPush ? (
                                <p className="text-sm text-muted-foreground">
                                    {headBranch?.upstream == null
                                        ? `The branch will be pushed to ${pushRemote} before the pull request opens.`
                                        : `${ahead} unpushed ${
                                              ahead === 1 ? "commit" : "commits"
                                          } will be pushed to ${pushRemote} first.`}
                                </p>
                            ) : null}
                        </>
                    ) : (
                        <p className="text-sm text-muted-foreground">
                            {notice}
                        </p>
                    )}
                </DialogPanel>
                <DialogFooter>
                    <DialogClose render={<Button variant="ghost" />}>
                        Cancel
                    </DialogClose>
                    <Button
                        type="submit"
                        disabled={!canSubmit}
                        loading={push.isPending || create.pending}
                    >
                        Create pull request
                    </Button>
                </DialogFooter>
            </Form>
        </>
    );
}
