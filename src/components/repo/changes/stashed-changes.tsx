import { ArchiveRestore, ChevronUp, Trash2 } from "lucide-react";
import type { ComponentType } from "react";
import { useMemo } from "react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
    Collapsible,
    CollapsibleContent,
    CollapsibleTrigger,
} from "@/components/ui/collapsible";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
    TooltipCreateHandle,
    TooltipPayloadHost,
    TooltipProvider,
    TooltipTrigger,
} from "@/components/ui/tooltip";
import { useConfirm } from "@/contexts/confirm-context";
import { useStashList } from "@/hooks/repositories/use-repository-queries";
import { useStashPopMutation } from "@/lib/backend/mutations/repository-mutations";
import type { StashEntry } from "@/lib/backend/protocol";
import { toastError } from "@/lib/toast-error";

const restorePayload = () => <span>Restore</span>;
const dropPayload = () => <span>Drop</span>;

/**
 * A collapsed "Stashed Changes" row pinned above the commit form, mirroring
 * GitHub Desktop. Expanding it lists stashes; rows restore (pop) or drop.
 */
export function StashedChanges({
    repoId,
    selectedId,
    onSelect,
}: {
    repoId: number;
    selectedId: string | null;
    onSelect: (id: string | null) => void;
}) {
    const stashes = useStashList(repoId);
    const pop = useStashPopMutation(repoId);
    const { confirm } = useConfirm();
    // Per instance: hidden repo tabs stay mounted, so a module-level handle
    // would be shared by several providers at once.
    const tooltip = useMemo(() => TooltipCreateHandle<ComponentType>(), []);

    const entries = stashes.data ?? [];
    if (entries.length === 0) return null;

    const restore = async (entry: StashEntry) => {
        try {
            await pop.mutateAsync({ index: entry.index, action: "pop" });
            if (selectedId === entry.commit) onSelect(null);
        } catch (error) {
            toastError("Could not restore stash", error);
        }
    };

    const drop = async (entry: StashEntry) => {
        const result = await confirm({
            title: `Drop stash@{${entry.index}}?`,
            description: "This cannot be undone.",
            confirmText: "Drop",
            variant: "destructive",
        });
        if (!result.confirmed) return;
        try {
            await pop.mutateAsync({ index: entry.index, action: "drop" });
            if (selectedId === entry.commit) onSelect(null);
        } catch (error) {
            toastError("Could not drop stash", error);
        }
    };

    return (
        <TooltipProvider>
            <Collapsible className="border-t" data-testid="stashed-changes">
                <CollapsibleTrigger
                    data-testid="stashed-changes-trigger"
                    className="flex w-full items-center gap-1 border-transparent px-1.5 py-1 text-foreground outline-none hover:bg-accent data-panel-open:[&_svg]:rotate-180"
                >
                    <ChevronUp className="mr-0.5 size-4 shrink-0 text-muted-foreground" />
                    <span className="font-semibold">Stashed Changes</span>
                    <Badge
                        className="h-6.5 rounded-md font-mono sm:h-5.5"
                        variant="outline"
                    >
                        {entries.length}
                    </Badge>
                </CollapsibleTrigger>
                <CollapsibleContent>
                    <ScrollArea
                        scrollFade
                        scrollX={false}
                        className="[&_[data-slot=scroll-area-viewport]]:max-h-40"
                    >
                        <div className="flex flex-col">
                            {entries.map((entry) => {
                                const selected = selectedId === entry.commit;
                                return (
                                    <div
                                        key={entry.index}
                                        role="option"
                                        aria-selected={selected}
                                        data-selected={selected || undefined}
                                        onClick={() =>
                                            onSelect(
                                                selected ? null : entry.commit
                                            )
                                        }
                                        className="flex cursor-pointer items-center justify-between gap-2 px-1.5 py-0.5 hover:bg-accent data-selected:bg-accent/64"
                                    >
                                        <div className="min-w-0 flex-1">
                                            <span className="block truncate text-sm">
                                                {entry.message}
                                            </span>
                                            <span className="block truncate font-mono text-xs text-muted-foreground">
                                                stash@{`{${entry.index}}`}
                                            </span>
                                        </div>
                                        <div
                                            className="flex shrink-0 items-center gap-0.5 pr-1"
                                            onClick={(event) =>
                                                event.stopPropagation()
                                            }
                                        >
                                            <TooltipTrigger
                                                handle={tooltip}
                                                payload={restorePayload}
                                                render={
                                                    <Button
                                                        size="icon-2xs"
                                                        variant="ghost"
                                                        disabled={pop.isPending}
                                                        aria-label={`Restore stash ${entry.index}`}
                                                        onClick={() =>
                                                            void restore(entry)
                                                        }
                                                    >
                                                        <ArchiveRestore />
                                                    </Button>
                                                }
                                            />
                                            <TooltipTrigger
                                                handle={tooltip}
                                                payload={dropPayload}
                                                render={
                                                    <Button
                                                        size="icon-2xs"
                                                        variant="ghost"
                                                        disabled={pop.isPending}
                                                        aria-label={`Drop stash ${entry.index}`}
                                                        onClick={() =>
                                                            void drop(entry)
                                                        }
                                                    >
                                                        <Trash2 />
                                                    </Button>
                                                }
                                            />
                                        </div>
                                    </div>
                                );
                            })}
                        </div>
                    </ScrollArea>
                </CollapsibleContent>
            </Collapsible>
            <TooltipPayloadHost handle={tooltip} />
        </TooltipProvider>
    );
}
