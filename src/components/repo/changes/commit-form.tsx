import { useState } from "react";
import type { FormEvent } from "react";

import { Button } from "@/components/ui/button";
import { Field, FieldLabel } from "@/components/ui/field";
import { Fieldset } from "@/components/ui/fieldset";
import { Form } from "@/components/ui/form";
import {
    InputGroup,
    InputGroupAddon,
    InputGroupInput,
} from "@/components/ui/input-group";
import { Textarea } from "@/components/ui/textarea";
import { useMergeActions } from "@/hooks/repositories/use-merge-actions";
import {
    useOperationState,
    useRepositoryStatus,
} from "@/hooks/repositories/use-repository-queries";
import { useCommitMutation } from "@/lib/backend/mutations/repository-mutations";
import type { HookRunResult } from "@/lib/backend/protocol";
import { toastError } from "@/lib/toast-error";

import { HookChecker } from "./hook-checker";

export function CommitForm({
    repoId,
    branch,
    stagedCount,
    onCommitted,
}: {
    repoId: number;
    branch: string;
    stagedCount: number;
    onCommitted?: () => void;
}) {
    const [summary, setSummary] = useState("");
    const [description, setDescription] = useState("");
    const [naturalHookRuns, setNaturalHookRuns] = useState<HookRunResult[]>([]);
    const commit = useCommitMutation(repoId);
    const operation = useOperationState(repoId);
    const status = useRepositoryStatus(repoId);
    const mergeActions = useMergeActions(repoId);

    const merging = operation.data?.kind === "merge";
    const blocked = merging && (status.data?.conflicts.length ?? 0) > 0;
    const busy = commit.isPending || mergeActions.pending;
    const canCommit =
        summary.trim().length > 0 && stagedCount > 0 && !busy && !blocked;

    const submit = async (event: FormEvent<HTMLFormElement>) => {
        event.preventDefault();
        if (!canCommit) return;
        const message = buildCommitMessage(summary, description);
        try {
            if (merging) {
                const outcome = await mergeActions.continueMerge(message);
                if (!outcome) return;
            } else {
                const execution = await commit.mutateAsync({ message });
                setNaturalHookRuns(execution.hookRuns);
            }
            setSummary("");
            setDescription("");
            onCommitted?.();
        } catch (error) {
            toastError(
                merging ? "Could not finish merge" : "Commit failed",
                error
            );
        }
    };

    return (
        <Form
            className="flex flex-col gap-1"
            onSubmit={(event) => void submit(event)}
            onKeyDown={(event) => {
                if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
                    event.currentTarget.requestSubmit();
                }
            }}
        >
            <Fieldset className="flex flex-col gap-1">
                <Field>
                    <FieldLabel className="sr-only">Commit summary</FieldLabel>
                    <InputGroup>
                        <InputGroupInput
                            id="commit-summary-input"
                            placeholder="Summary (required)"
                            type="text"
                            value={summary}
                            onChange={(e) => setSummary(e.target.value)}
                            disabled={busy}
                        />
                        <InputGroupAddon align="inline-end" className="gap-0.5">
                            <HookChecker
                                repoId={repoId}
                                naturalRuns={
                                    naturalHookRuns.length > 0
                                        ? naturalHookRuns
                                        : undefined
                                }
                            />
                        </InputGroupAddon>
                    </InputGroup>
                </Field>
                <Field>
                    <FieldLabel className="sr-only">
                        Commit description
                    </FieldLabel>
                    <Textarea
                        placeholder="Description"
                        value={description}
                        onChange={(e) => setDescription(e.target.value)}
                        disabled={busy}
                    />
                </Field>
            </Fieldset>
            <Button
                variant="info"
                className="w-full text-background dark:text-foreground"
                type="submit"
                loading={busy}
                disabled={!canCommit}
            >
                {merging ? `Finish merge to ${branch}` : `Commit to ${branch}`}
            </Button>
            {merging && (
                <Button
                    variant="ghost"
                    size="xs"
                    className="w-full text-muted-foreground"
                    disabled={busy}
                    onClick={() => void mergeActions.abortMerge()}
                >
                    Abort merge
                </Button>
            )}
        </Form>
    );
}

function buildCommitMessage(summary: string, description: string): string {
    const trimmedSummary = summary.trim();
    const trimmedDescription = description.trim();
    return trimmedDescription.length > 0
        ? `${trimmedSummary}\n\n${trimmedDescription}`
        : trimmedSummary;
}
