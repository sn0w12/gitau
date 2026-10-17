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
    SelectGroup,
    SelectGroupLabel,
    SelectItem,
    SelectPopup,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import {
    usePublishOwnerGroups,
    type PublishOwnerGroup,
} from "@/hooks/forge/use-forge-account";
import { repoDisplayName } from "@/hooks/repositories/use-repo-identity";
import { usePublishRepositoryMutation } from "@/lib/backend/mutations/repository-mutations";
import type { PublishResult } from "@/lib/backend/protocol";
import { GitBackendError } from "@/lib/backend/transport/invoke";
import { toastError } from "@/lib/toast-error";

/** Splits a `provider/owner` selection, where an empty owner is the personal
 * account. Org logins never contain a slash. */
function parseTarget(
    value: string | null
): { provider: string; owner: string | null } | null {
    if (value == null) return null;
    const [provider, ...rest] = value.split("/");
    const owner = rest.join("/");
    return { provider, owner: owner === "" ? null : owner };
}

/**
 * Creates the repository on the chosen provider: owner (personal or org),
 * name, description, visibility. On success the backend has already wired up
 * `origin` and pushed the current branch.
 *
 * Always mounted and driven by `open`; the form and its queries live inside
 * the portal body, so they mount fresh on every open.
 */
export function PublishRepositoryDialog({
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
                <PublishRepositoryDialogBody
                    repoId={repoId}
                    repoPath={repoPath}
                    onClose={onClose}
                />
            </DialogPopup>
        </Dialog>
    );
}

function PublishRepositoryDialogBody({
    repoId,
    repoPath,
    onClose,
}: {
    repoId: number;
    repoPath: string;
    onClose: () => void;
}) {
    const { groups, hasPublishProvider } = usePublishOwnerGroups();
    const [selected, setSelected] = useState<string | null>(null);
    const [name, setName] = useState<string>(() => repoDisplayName(repoPath));
    const [description, setDescription] = useState("");
    const [isPrivate, setIsPrivate] = useState(true);
    const [conflict, setConflict] = useState<string | null>(null);

    const mutation = usePublishRepositoryMutation(repoId);

    const offered = groups.filter((group) => group.entries.length > 0);
    const value = selected ?? offered[0]?.entries[0]?.value ?? null;
    const target = parseTarget(value);
    const activeGroup = groups.find(
        (group) => group.provider === target?.provider
    );
    const items = offered.map((group) => ({
        label: group.label,
        items: group.entries.map((entry) => ({
            label: entry.label,
            value: entry.value,
        })),
    }));

    const handleSubmit = async () => {
        if (!canSubmit) return;
        setConflict(null);
        try {
            const result: PublishResult = await mutation.mutateAsync({
                provider: target?.provider,
                owner: target?.owner ?? undefined,
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
                        target?.owner ? ` under ${target.owner}` : ""
                    }. Pick another one.`
                );
                return undefined;
            }
            toastError("Publishing failed", error);
            return undefined;
        }
    };

    const canSubmit =
        target != null && name.trim().length > 0 && !mutation.isPending;

    const notice = !hasPublishProvider
        ? "This build has no provider that can create repositories."
        : groups.length === 1
          ? `Connect a ${groups[0].label} account first: open Account from the sidebar and sign in.`
          : "Connect an account first: open Account from the sidebar and sign in.";

    return (
        <>
            <DialogHeader>
                <DialogTitle>Publish repository</DialogTitle>
                <DialogDescription>
                    {activeGroup
                        ? `Creates the repository on ${activeGroup.label}, points origin at it, and pushes your current branch.`
                        : "Creates the repository, points origin at it, and pushes your current branch."}
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
                    {target == null || activeGroup == null ? (
                        <p className="text-sm text-muted-foreground">
                            {notice}
                        </p>
                    ) : (
                        <>
                            <Field>
                                <FieldLabel>Owner</FieldLabel>
                                <Select
                                    value={value ?? ""}
                                    onValueChange={(next) => {
                                        if (typeof next === "string")
                                            setSelected(next);
                                    }}
                                    items={items}
                                >
                                    <SelectTrigger className="w-full">
                                        <SelectValue>
                                            {() =>
                                                ownerLabel(activeGroup, value)
                                            }
                                        </SelectValue>
                                    </SelectTrigger>
                                    <SelectPopup>
                                        {items.map((group) => (
                                            <SelectGroup key={group.label}>
                                                <SelectGroupLabel>
                                                    {group.label}
                                                </SelectGroupLabel>
                                                {group.items.map((entry) => (
                                                    <SelectItem
                                                        key={entry.value}
                                                        value={entry.value}
                                                    >
                                                        {entry.label}
                                                    </SelectItem>
                                                ))}
                                            </SelectGroup>
                                        ))}
                                    </SelectPopup>
                                </Select>
                            </Field>
                            <Field>
                                <FieldLabel>Name</FieldLabel>
                                <Input
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
                            </Field>
                            <Field>
                                <FieldLabel>Description</FieldLabel>
                                <Input
                                    value={description}
                                    onChange={(event) =>
                                        setDescription(event.target.value)
                                    }
                                    placeholder="Optional"
                                />
                            </Field>
                            <Field>
                                <FieldLabel>
                                    <Checkbox
                                        checked={isPrivate}
                                        onCheckedChange={(checked) =>
                                            setIsPrivate(checked === true)
                                        }
                                    />
                                    Private
                                </FieldLabel>
                            </Field>
                        </>
                    )}
                </DialogPanel>
                <DialogFooter>
                    <DialogClose render={<Button variant="ghost" />}>
                        Cancel
                    </DialogClose>
                    <Button
                        type="submit"
                        disabled={!canSubmit}
                        loading={mutation.isPending}
                    >
                        Create repository
                    </Button>
                </DialogFooter>
            </Form>
        </>
    );
}

/** The closed control names the provider too, since the owner alone does not
 * say which account it belongs to once two are connected. */
function ownerLabel(group: PublishOwnerGroup, value: string | null): string {
    const entry = group.entries.find((item) => item.value === value);
    return entry ? `${group.label} · ${entry.label}` : group.label;
}

function isNameTaken(error: unknown): boolean {
    if (!(error instanceof GitBackendError)) return false;
    return (
        error.code === "forge" &&
        /already exist/i.test(error.detail ?? error.message)
    );
}
