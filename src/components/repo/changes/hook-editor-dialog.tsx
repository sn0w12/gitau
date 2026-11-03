import { SquarePen } from "lucide-react";
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
import { Field, FieldLabel } from "@/components/ui/field";
import { Form } from "@/components/ui/form";
import { Tabs, TabsList, TabsPanel, TabsTab } from "@/components/ui/tabs";
import { ScrollTextarea } from "@/components/ui/textarea";
import {
    Tooltip,
    TooltipTrigger,
    TooltipContent,
} from "@/components/ui/tooltip";
import { useAppServices } from "@/contexts/services-context";
import { useOpenInEditor } from "@/hooks/repositories/use-open-in-editor";
import { repositoryKeys } from "@/lib/backend/queries/query-keys";
import { toastError } from "@/lib/toast-error";

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
    open,
    onClose,
}: {
    repoId: number;
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
            <DialogPopup className="max-w-3xl" data-testid="hook-editor-dialog">
                <HookEditorDialogBody repoId={repoId} />
            </DialogPopup>
        </Dialog>
    );
}

function HookEditorDialogBody({ repoId }: { repoId: number }) {
    const { backend, queryClient } = useAppServices();
    const { enabled: editorConfigured, openInEditor } = useOpenInEditor();
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
            <Form
                className="contents"
                onSubmit={(event) => {
                    event.preventDefault();
                    void handleSave();
                }}
            >
                <DialogPanel>
                    <Tabs
                        orientation="vertical"
                        value={activeHook}
                        onValueChange={(value) => {
                            if (typeof value === "string") setActiveHook(value);
                        }}
                        className="gap-4"
                    >
                        <TabsList
                            aria-label="Commit hooks"
                            className="h-fit shrink-0 self-start p-1"
                        >
                            {STANDARD_HOOKS.map((name) => (
                                <TabsTab
                                    key={name}
                                    value={name}
                                    className="flex flex-row px-1.5 text-xs data-[orientation=vertical]:justify-between"
                                >
                                    <span className="min-w-0 truncate">
                                        {name}
                                    </span>
                                </TabsTab>
                            ))}
                        </TabsList>
                        {STANDARD_HOOKS.map((name) => {
                            const loaded = byHook[name];
                            const canOpenInEditor =
                                editorConfigured && loaded?.exists === true;
                            return (
                                <TabsPanel
                                    key={name}
                                    value={name}
                                    className="min-w-0"
                                >
                                    <Field className="gap-1.5">
                                        <div className="flex w-full items-center justify-between">
                                            <FieldLabel className="ui-selectable cursor-text">
                                                {name} script
                                            </FieldLabel>
                                            <Tooltip>
                                                <TooltipTrigger
                                                    render={
                                                        <Button
                                                            size="icon-2xs"
                                                            variant="ghost"
                                                            aria-label={`Open ${name} in editor`}
                                                            disabled={
                                                                !canOpenInEditor
                                                            }
                                                            onClick={() => {
                                                                if (loaded)
                                                                    void openInEditor(
                                                                        loaded.path
                                                                    );
                                                            }}
                                                        >
                                                            <SquarePen />
                                                        </Button>
                                                    }
                                                />
                                                <TooltipContent>
                                                    Open in editor
                                                </TooltipContent>
                                            </Tooltip>
                                        </div>
                                        {loaded == null ? (
                                            <ScrollTextarea
                                                className="h-[12lh] font-mono"
                                                disabled
                                            />
                                        ) : (
                                            <ScrollTextarea
                                                value={loaded.content}
                                                onChange={(event) =>
                                                    setByHook((prev) => ({
                                                        ...prev,
                                                        [name]: {
                                                            ...prev[name],
                                                            content:
                                                                event.target
                                                                    .value,
                                                        },
                                                    }))
                                                }
                                                spellCheck={false}
                                                className="h-[12lh] font-mono"
                                            />
                                        )}
                                    </Field>
                                </TabsPanel>
                            );
                        })}
                    </Tabs>
                </DialogPanel>
                <DialogFooter>
                    <DialogClose render={<Button variant="ghost" />}>
                        Cancel
                    </DialogClose>
                    <Button type="submit" disabled={!canSave} loading={saving}>
                        Save
                    </Button>
                </DialogFooter>
            </Form>
        </>
    );
}
