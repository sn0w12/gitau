import { Check } from "lucide-react";
import { useEffect, useState } from "react";

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
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useAppServices } from "@/contexts/services-context";
import type { GitHook } from "@/lib/backend/protocol";
import { repositoryKeys } from "@/lib/backend/queries/query-keys";
import { toastError } from "@/lib/toast-error";
import { cn } from "@/lib/utils";

/** Standard commit-lifecycle hooks in pipeline order, mirroring
 * `COMMIT_HOOK_NAMES` in crates/git-backend/src/engines/git2/hooks.rs; the
 * backend rejects any other name. */
const STANDARD_HOOKS = [
    "pre-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
] as const;

interface HookEditorEntry {
    /** Editable script text. */
    content: string;
    /** Content as last read or saved; the dirty baseline. */
    onDisk: string;
    path: string;
    exists: boolean;
}

/**
 * Edits commit hook scripts in place: pick one of the four standard
 * lifecycle hooks, installed or not, edit its script, save. Always mounted
 * and driven by `open`; the body lives inside the portal, so it remounts
 * fresh on every open.
 */
export function HookEditorDialog({
    repoId,
    hooks,
    open,
    onClose,
}: {
    repoId: number;
    hooks: GitHook[];
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
            <DialogPopup className="max-w-xl" data-testid="hook-editor-dialog">
                <HookEditorDialogBody repoId={repoId} hooks={hooks} />
            </DialogPopup>
        </Dialog>
    );
}

function HookEditorDialogBody({
    repoId,
    hooks,
}: {
    repoId: number;
    hooks: GitHook[];
}) {
    const { backend, queryClient } = useAppServices();
    const [activeHook, setActiveHook] = useState<string>(STANDARD_HOOKS[0]);
    const [byHook, setByHook] = useState<Record<string, HookEditorEntry>>({});
    const [saving, setSaving] = useState(false);

    // Read every standard hook once on open, so switching between scripts
    // never waits on the backend again.
    useEffect(() => {
        let cancelled = false;
        void (async () => {
            const outcomes = await Promise.all(
                STANDARD_HOOKS.map(async (hook) => ({
                    hook,
                    outcome: await backend.hooks.read(repoId, hook),
                }))
            );
            if (cancelled) return;
            const loaded: Record<string, HookEditorEntry> = {};
            for (const { hook, outcome } of outcomes) {
                if (!outcome.ok) {
                    toastError(`Could not read ${hook}`, outcome.error);
                    continue;
                }
                const value = outcome.value;
                loaded[value.hook] = {
                    content: value.content,
                    onDisk: value.content,
                    path: value.path,
                    exists: value.exists,
                };
            }
            setByHook(loaded);
        })();
        return () => {
            cancelled = true;
        };
    }, [backend, repoId]);

    const entry = byHook[activeHook];
    const loading = entry == null;
    const dirty = entry != null && entry.content !== entry.onDisk;
    const canSave = !loading && !saving && dirty;

    const handleSave = async () => {
        if (entry == null || !canSave) return;
        setSaving(true);
        const outcome = await backend.hooks.write(
            repoId,
            activeHook,
            entry.content
        );
        setSaving(false);
        if (!outcome.ok) {
            toastError(`Could not save ${activeHook}`, outcome.error);
            return;
        }
        setByHook((prev) => ({
            ...prev,
            [activeHook]: {
                ...prev[activeHook],
                onDisk: prev[activeHook].content,
                exists: true,
            },
        }));
        // The popover lists hooks from this query; refresh so a newly
        // created hook shows up without reopening the popover.
        await queryClient.invalidateQueries({
            queryKey: repositoryKeys.hooks(repoId),
        });
    };

    return (
        <>
            <DialogHeader>
                <DialogTitle>Edit commit hooks</DialogTitle>
                <DialogDescription>
                    Scripts git runs around each commit. Missing hooks are
                    created on save.
                </DialogDescription>
            </DialogHeader>
            <DialogPanel className="grid grid-cols-[9.5rem_1fr] gap-4">
                <ul className="flex flex-col gap-1">
                    {STANDARD_HOOKS.map((name) => (
                        <HookPickerRow
                            key={name}
                            name={name}
                            hook={hooks.find((hook) => hook.name === name)}
                            active={activeHook === name}
                            onSelect={() => setActiveHook(name)}
                        />
                    ))}
                </ul>
                <div className="flex min-w-0 flex-col gap-1.5">
                    <Label htmlFor="hook-editor-content">
                        {activeHook} script
                    </Label>
                    {loading ? (
                        <Textarea rows={12} disabled />
                    ) : (
                        <Textarea
                            id="hook-editor-content"
                            value={entry.content}
                            onChange={(event) =>
                                setByHook((prev) => ({
                                    ...prev,
                                    [activeHook]: {
                                        ...prev[activeHook],
                                        content: event.target.value,
                                    },
                                }))
                            }
                            rows={12}
                            spellCheck={false}
                            autoFocus
                            className="font-mono"
                        />
                    )}
                    <p className="truncate font-mono text-xs text-muted-foreground">
                        {entry?.path}
                    </p>
                </div>
            </DialogPanel>
            <DialogFooter>
                <DialogClose render={<Button variant="ghost" />}>
                    Cancel
                </DialogClose>
                <Button
                    disabled={!canSave}
                    loading={saving}
                    onClick={() => void handleSave()}
                >
                    Save
                </Button>
            </DialogFooter>
        </>
    );
}

function HookPickerRow({
    name,
    hook,
    active,
    onSelect,
}: {
    name: string;
    hook: GitHook | undefined;
    active: boolean;
    onSelect: () => void;
}) {
    return (
        <li>
            <button
                type="button"
                aria-pressed={active}
                onClick={onSelect}
                className={cn(
                    "flex w-full cursor-pointer items-center gap-1.5 rounded-md px-1.5 py-1 text-left text-xs",
                    active ? "bg-accent" : "hover:bg-accent/60"
                )}
            >
                <span className="min-w-0 truncate font-mono">{name}</span>
                {hook && (
                    <Check
                        data-testid={`hook-picker-${name}-installed`}
                        className="ml-auto size-3 shrink-0 text-success"
                    />
                )}
            </button>
        </li>
    );
}
