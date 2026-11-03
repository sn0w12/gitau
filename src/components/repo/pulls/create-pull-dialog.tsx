import { Tag } from "lucide-react";
import { useMemo, useState } from "react";

import { LabelBadge } from "@/components/forge/label-badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
    Combobox,
    ComboboxChip,
    ComboboxChips,
    ComboboxChipsInput,
    ComboboxEmpty,
    ComboboxItem,
    ComboboxList,
    ComboboxPopup,
    ComboboxValue,
} from "@/components/ui/combobox";
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
    useRepoLabels,
} from "@/hooks/forge/use-forge-issues";
import { useCreatePullRequest } from "@/hooks/forge/use-forge-pull-requests";
import {
    useRepoListing,
    useRepositorySnapshot,
} from "@/hooks/repositories/use-repository-queries";
import { useActiveTabRouter } from "@/hooks/tabs/use-active-tab-router";
import { usePushMutation } from "@/lib/backend/mutations/repository-mutations";
import type {
    BranchInfo,
    Capabilities,
    ProviderDescriptor,
    RemoteInfo,
} from "@/lib/backend/protocol";
import { pickRepoRemote, type RemoteCoords } from "@/lib/forge/repo-coords";
import { pickBaseBranch } from "@/lib/repositories/default-branch";
import { toastError } from "@/lib/toast-error";

export function CreatePullDialog({
    repoId,
    initialHead,
    open,
    onClose,
}: {
    repoId: number;
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

type LabelOption = { value: string; label: string; color: string };

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
    const target = forgeTarget(coords);
    const providers = useForgeProviders();
    const capabilities = useRepoCapabilities(repoId);
    const account = useForgeAccount(target.provider);
    const listing = useRepoListing(repoId);
    const snapshot = useRepositorySnapshot(repoId);
    const labelOptions = useRepoLabels(
        target.provider,
        target.owner,
        target.repo
    );
    const push = usePushMutation(repoId);
    const create = useCreatePullRequest();
    const router = useActiveTabRouter();

    const branches = listing.data?.branches ?? [];
    const names = branches.map((branch) => branch.name);
    const currentName =
        snapshot.data?.head.state === "attached"
            ? snapshot.data.head.branch
            : null;
    const labelItems = useMemo<LabelOption[]>(
        () =>
            (labelOptions.data ?? []).map((label) => ({
                value: label.name,
                label: label.name,
                color: label.color,
            })),
        [labelOptions.data]
    );

    const [head, setHead] = useState<string | null>(null);
    const [base, setBase] = useState<string | null>(null);
    const [title, setTitle] = useState<string | null>(null);
    const [description, setDescription] = useState("");
    const [draft, setDraft] = useState(false);
    const [labels, setLabels] = useState<LabelOption[]>([]);

    const { headValue, baseValue, titleValue } = resolveDraftValues({
        head,
        base,
        title,
        initialHead,
        names,
        currentName,
    });

    const headBranch = branches.find((branch) => branch.name === headValue);
    const pushRemote = pushRemoteName(remotes, providers.data);
    const pushNotice = pushNoticeText(headBranch, pushRemote);
    const providerName = providerDisplayName(providers.data, target.provider);
    const notice = pullBlockReason({
        hasCoords: coords != null,
        providerName,
        capabilities,
        connected: account.data != null,
        branchCount: names.length,
    });
    const busy = push.isPending || create.pending;
    const canSubmit = canCreatePull({
        notice,
        head: headValue,
        base: baseValue,
        title: titleValue,
        busy,
    });

    const handleSubmit = async () => {
        if (!canSubmit || coords == null) return;
        try {
            if (pushNotice != null) {
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
                    labels: labels.map((label) => label.value),
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
                            <BranchFields
                                names={names}
                                head={headValue}
                                onHeadChange={(next) => {
                                    setHead(next);
                                    if (baseValue === next) setBase(null);
                                }}
                                base={baseValue}
                                onBaseChange={setBase}
                            />
                            <PullDetailsFields
                                title={titleValue}
                                onTitleChange={setTitle}
                                description={description}
                                onDescriptionChange={setDescription}
                                labelItems={labelItems}
                                labels={labels}
                                onLabelsChange={setLabels}
                                draft={draft}
                                onDraftChange={setDraft}
                            />
                            {pushNotice != null && (
                                <p className="text-sm text-muted-foreground">
                                    {pushNotice}
                                </p>
                            )}
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
                    <Button type="submit" disabled={!canSubmit} loading={busy}>
                        Create pull request
                    </Button>
                </DialogFooter>
            </Form>
        </>
    );
}

function BranchFields({
    names,
    head,
    base,
    onHeadChange,
    onBaseChange,
}: {
    names: string[];
    head: string;
    base: string;
    onHeadChange: (next: string) => void;
    onBaseChange: (next: string) => void;
}) {
    const baseNames = names.filter((name) => name !== head);
    return (
        <div className="grid grid-cols-2 gap-3">
            <Field>
                <FieldLabel>Head</FieldLabel>
                <Select
                    value={head}
                    onValueChange={(next) => {
                        if (typeof next === "string") onHeadChange(next);
                    }}
                    items={branchItems(names)}
                >
                    <SelectTrigger className="w-full">
                        <SelectValue />
                    </SelectTrigger>
                    <SelectPopup>
                        {names.map((name) => (
                            <SelectItem key={name} value={name}>
                                {name}
                            </SelectItem>
                        ))}
                    </SelectPopup>
                </Select>
            </Field>
            <Field>
                <FieldLabel>Base</FieldLabel>
                <Select
                    value={base}
                    onValueChange={(next) => {
                        if (typeof next === "string") onBaseChange(next);
                    }}
                    items={branchItems(baseNames)}
                >
                    <SelectTrigger className="w-full">
                        <SelectValue />
                    </SelectTrigger>
                    <SelectPopup>
                        {baseNames.map((name) => (
                            <SelectItem key={name} value={name}>
                                {name}
                            </SelectItem>
                        ))}
                    </SelectPopup>
                </Select>
            </Field>
        </div>
    );
}

function PullDetailsFields({
    title,
    onTitleChange,
    description,
    onDescriptionChange,
    labelItems,
    labels,
    onLabelsChange,
    draft,
    onDraftChange,
}: {
    title: string;
    onTitleChange: (next: string) => void;
    description: string;
    onDescriptionChange: (next: string) => void;
    labelItems: LabelOption[];
    labels: LabelOption[];
    onLabelsChange: (next: LabelOption[]) => void;
    draft: boolean;
    onDraftChange: (next: boolean) => void;
}) {
    return (
        <>
            <Field>
                <FieldLabel>Title</FieldLabel>
                <Input
                    value={title}
                    onChange={(event) => onTitleChange(event.target.value)}
                    autoFocus
                />
            </Field>
            <Field>
                <FieldLabel>Description</FieldLabel>
                <Textarea
                    value={description}
                    onChange={(event) =>
                        onDescriptionChange(event.target.value)
                    }
                    rows={4}
                    placeholder="Optional"
                />
            </Field>
            <Field>
                <FieldLabel>Labels</FieldLabel>
                <Combobox
                    multiple
                    items={labelItems}
                    value={labels}
                    onValueChange={(value) =>
                        onLabelsChange(value as LabelOption[])
                    }
                >
                    <ComboboxChips startAddon={<Tag />}>
                        <ComboboxValue>
                            {(value: LabelOption[]) => (
                                <>
                                    {value.map((label) => (
                                        <ComboboxChip
                                            key={label.value}
                                            aria-label={label.label}
                                        >
                                            <LabelBadge
                                                label={{
                                                    name: label.label,
                                                    color: label.color,
                                                }}
                                            />
                                        </ComboboxChip>
                                    ))}
                                    <ComboboxChipsInput
                                        aria-label="Select labels"
                                        placeholder={
                                            value.length > 0
                                                ? undefined
                                                : "None"
                                        }
                                    />
                                </>
                            )}
                        </ComboboxValue>
                    </ComboboxChips>
                    <ComboboxPopup>
                        <ComboboxEmpty>No labels found.</ComboboxEmpty>
                        <ComboboxList>
                            {(item: LabelOption) => (
                                <ComboboxItem key={item.value} value={item}>
                                    <LabelBadge
                                        label={{
                                            name: item.label,
                                            color: item.color,
                                        }}
                                    />
                                </ComboboxItem>
                            )}
                        </ComboboxList>
                    </ComboboxPopup>
                </Combobox>
            </Field>
            <Field>
                <FieldLabel>
                    <Checkbox
                        checked={draft}
                        onCheckedChange={(checked) =>
                            onDraftChange(checked === true)
                        }
                    />
                    Draft
                </FieldLabel>
            </Field>
        </>
    );
}

function forgeTarget(coords: RemoteCoords | null): {
    provider: string | null;
    owner: string | null;
    repo: string | null;
} {
    return {
        provider: coords?.provider ?? null,
        owner: coords?.owner ?? null,
        repo: coords?.repo ?? null,
    };
}

function resolveDraftValues({
    head,
    base,
    title,
    initialHead,
    names,
    currentName,
}: {
    head: string | null;
    base: string | null;
    title: string | null;
    initialHead: string | undefined;
    names: string[];
    currentName: string | null;
}): { headValue: string; baseValue: string; titleValue: string } {
    const headValue = head ?? defaultHead(initialHead, names, currentName);
    return {
        headValue,
        baseValue: base ?? pickBaseBranch(names, headValue),
        titleValue: title ?? headValue,
    };
}

function defaultHead(
    initialHead: string | undefined,
    names: string[],
    currentName: string | null
): string {
    if (initialHead != null && names.includes(initialHead)) return initialHead;
    return currentName ?? names[0] ?? "";
}

function pushRemoteName(
    remotes: RemoteInfo[] | undefined,
    providers: ProviderDescriptor[] | undefined
): string {
    return pickRepoRemote(remotes ?? [], providers ?? [])?.name ?? "origin";
}

function pushNoticeText(
    head: BranchInfo | undefined,
    remote: string
): string | null {
    if (head == null) return null;
    if (head.upstream == null) {
        return `The branch will be pushed to ${remote} before the pull request opens.`;
    }
    const ahead = head.upstream.ahead;
    if (ahead === 0) return null;
    return `${ahead} unpushed ${
        ahead === 1 ? "commit" : "commits"
    } will be pushed to ${remote} first.`;
}

function providerDisplayName(
    providers: ProviderDescriptor[] | undefined,
    providerId: string | null
): string {
    if (providerId == null) return "the provider";
    const known = providers?.find((entry) => entry.id === providerId)?.label;
    return known ?? providerId;
}

function pullBlockReason({
    hasCoords,
    providerName,
    capabilities,
    connected,
    branchCount,
}: {
    hasCoords: boolean;
    providerName: string;
    capabilities: Capabilities | null;
    connected: boolean;
    branchCount: number;
}): string | null {
    if (!hasCoords) {
        return "No remote of this repository points at a provider this build knows, so there is nowhere to open a pull request.";
    }
    if (capabilities != null && !capabilities.pullRequestsWrite) {
        return `${providerName} cannot open pull requests from this build.`;
    }
    if (!connected) {
        return `Connect a ${providerName} account first: open Account from the sidebar and sign in.`;
    }
    if (branchCount < 2) {
        return "This repository needs a second branch to merge into.";
    }
    return null;
}

function canCreatePull({
    notice,
    head,
    base,
    title,
    busy,
}: {
    notice: string | null;
    head: string;
    base: string;
    title: string;
    busy: boolean;
}): boolean {
    return (
        notice == null &&
        head !== "" &&
        base !== "" &&
        base !== head &&
        title.trim().length > 0 &&
        !busy
    );
}

function branchItems(names: string[]): { value: string; label: string }[] {
    return names.map((name) => ({ value: name, label: name }));
}
