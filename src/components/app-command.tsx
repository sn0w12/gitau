import type { RegisterableHotkey } from "@tanstack/react-hotkeys";
import { useSelector } from "@tanstack/react-store";
import { ArrowUpIcon, ArrowDownIcon, CornerDownLeftIcon } from "lucide-react";
import { Fragment, useMemo, useState } from "react";

import { StashChangesDialog } from "@/components/repo/changes/stash-changes-dialog";
import { useAppCommands } from "@/contexts/app-command-context";
import { useChangeActions } from "@/hooks/repositories/use-change-actions";
import { useOpenRepository } from "@/hooks/repositories/use-open-repository";
import {
    repoDisplayName,
    useRepoIdentities,
} from "@/hooks/repositories/use-repo-identity";
import { useSyncActions } from "@/hooks/repositories/use-sync-actions";
import { useUndoLastCommit } from "@/hooks/repositories/use-undo-last-commit";
import { useSettingHotkey } from "@/hooks/settings/use-setting-hotkey";
import { useActiveTabRouter } from "@/hooks/tabs/use-active-tab-router";
import { useStashPopMutation } from "@/lib/backend/mutations/repository-mutations";
import { toastError } from "@/lib/toast-error";
import { cn } from "@/lib/utils";
import { activateTab, appStore, selectTabs } from "@/stores/app-store";
import { repositoryStore } from "@/stores/repository-store";

import {
    Command,
    CommandCollection,
    CommandDialog,
    CommandDialogPopup,
    CommandEmpty,
    CommandFooter,
    CommandGroup,
    CommandGroupLabel,
    CommandInput,
    CommandItem,
    CommandList,
    CommandPanel,
    CommandSeparator,
} from "./ui/command";
import { ContextMenuShortcut } from "./ui/context-menu";
import { Kbd, KbdGroup, type AnyShortcut } from "./ui/kbd";

type Item =
    | {
          value: string;
          label: string;
          shortcut?: AnyShortcut;
          action?: () => unknown;
      }
    | {
          value: "separator";
      };

interface Group {
    value: string;
    items: Item[];
    visibleRoutes?: string[];
}

function isSeparator(item: Item): item is { value: "separator" } {
    return item.value === "separator";
}

function repoIdFromPath(pathname: string): number | undefined {
    const match = /^\/repo\/(\d+)(?:$|\/)/.exec(pathname);
    if (!match) return undefined;
    const repoId = Number(match[1]);
    return Number.isInteger(repoId) && repoId > 0 ? repoId : undefined;
}

export function AppCommand() {
    const [open, setOpen] = useState(false);
    const tabs = useSelector(appStore, selectTabs);
    const repos = useSelector(repositoryStore, (state) => [
        ...state.entries.values(),
    ]);

    const router = useActiveTabRouter();
    const pathname = router?.state.location.pathname ?? "/";
    useSettingHotkey("openCommand", () => setOpen(!open));

    const openRepo = useOpenRepository();
    const { invoke } = useAppCommands();

    const ownerInputs = useMemo(
        () =>
            repos.map((repo) => ({
                repoId: repo.repoId || 0,
                path: repo.path,
            })),
        [repos]
    );
    const owners = useRepoIdentities(ownerInputs);

    const paletteRepoId = repoIdFromPath(pathname);
    const repoEntry =
        paletteRepoId === undefined
            ? undefined
            : repos.find((repo) => repo.repoId === paletteRepoId);
    const changes = useChangeActions(paletteRepoId);
    const sync = useSyncActions(paletteRepoId ?? 0, repoEntry?.path ?? "");
    const undo = useUndoLastCommit(paletteRepoId);
    const stashPop = useStashPopMutation(paletteRepoId ?? 0);
    const [stashRepoId, setStashRepoId] = useState<number | null>(null);

    const handleAction = (item: Item) => {
        if (isSeparator(item)) return;
        item.action?.();
        setOpen(false);
    };

    const repo: Item[] = [
        {
            value: "clone-repo",
            label: "Clone Repo",
            action: () => invoke("cloneRepository"),
        },
        {
            value: "add-repo",
            label: "Add Existing Repo",
            action: () => invoke("addRepository"),
        },
        {
            value: "new-repo",
            label: "New Repo",
            action: () => invoke("newRepository"),
        },
        {
            value: "separator",
        },
        ...repos.map((repo) => {
            const owner = owners.get(repo.path);
            const prefix = owner ? `${owner}/` : "";
            return {
                value: repo.path,
                label: `Open ${prefix}${repoDisplayName(repo.path)}`,
                action: () => openRepo(repo.path),
            };
        }),
    ];
    const navigate: Item[] = [
        {
            value: "home",
            label: "Home",
            action: () => router?.navigate({ to: "/" }),
        },
        {
            value: "settings",
            label: "Settings",
            shortcut: "goToSettings",
            action: () => router?.navigate({ to: "/settings" }),
        },
        ...tabs.map((tab, index) => {
            return {
                value: tab.tabId,
                label: `Go to ${tab.title}`,
                shortcut:
                    index < 9
                        ? (`Mod+${index + 1}` as RegisterableHotkey)
                        : undefined,
                action: () => activateTab(tab.tabId),
            };
        }),
    ];
    const git: Item[] = [
        {
            value: "fetch",
            label: "Fetch Origin",
            shortcut: "fetchShortcut",
            action: () => void sync.fetch(),
        },
        {
            value: "undo-last-commit",
            label: "Undo Last Commit",
            action: () => void undo(),
        },
        {
            value: "stage-all",
            label: "Stage All Changes",
            shortcut: "stageAllShortcut",
            action: () => void changes.stageAll(),
        },
        {
            value: "unstage-all",
            label: "Unstage All Changes",
            shortcut: "unstageAllShortcut",
            action: () => void changes.unstageAll(),
        },
        {
            value: "discard-all",
            label: "Discard All Changes",
            shortcut: "discardAllShortcut",
            action: () => void changes.discardAll(),
        },
        {
            value: "stash",
            label: "Stash All Changes",
            action: () => setStashRepoId(paletteRepoId ?? null),
        },
        {
            value: "pop-stash",
            label: "Pop Latest Stash",
            action: () =>
                void stashPop
                    .mutateAsync({ index: 0, action: "pop" })
                    .catch((error) =>
                        toastError("Could not pop latest stash", error)
                    ),
        },
    ];

    const groupedItems: Group[] = [
        { items: repo, value: "Repo" },
        { items: navigate, value: "Navigate" },
        { items: git, value: "Git", visibleRoutes: ["/repo"] },
    ];

    const visibleGroups = groupedItems.filter(
        (group) =>
            !group.visibleRoutes ||
            group.visibleRoutes.some(
                (route) =>
                    pathname === route || pathname.startsWith(`${route}/`)
            )
    );

    return (
        <>
            <CommandDialog onOpenChange={setOpen} open={open}>
                <CommandDialogPopup>
                    <Command items={visibleGroups}>
                        <CommandInput placeholder="Search for apps and commands..." />
                        <CommandPanel>
                            <CommandEmpty>No results found.</CommandEmpty>
                            <CommandList>
                                {(group: Group, _index: number) => (
                                    <Fragment key={group.value}>
                                        <CommandGroup
                                            items={group.items}
                                            className={cn()}
                                        >
                                            <CommandGroupLabel>
                                                {group.value}
                                            </CommandGroupLabel>
                                            <CommandCollection>
                                                {(item: Item) => {
                                                    return isSeparator(item) ? (
                                                        <CommandSeparator />
                                                    ) : (
                                                        <CommandItem
                                                            key={item.value}
                                                            value={item.value}
                                                            onClick={() =>
                                                                handleAction(
                                                                    item
                                                                )
                                                            }
                                                        >
                                                            <span className="flex-1">
                                                                {item.label}
                                                            </span>
                                                            {item.shortcut && (
                                                                <ContextMenuShortcut
                                                                    shortcut={
                                                                        item.shortcut
                                                                    }
                                                                />
                                                            )}
                                                        </CommandItem>
                                                    );
                                                }}
                                            </CommandCollection>
                                        </CommandGroup>
                                        <CommandSeparator />
                                    </Fragment>
                                )}
                            </CommandList>
                        </CommandPanel>
                        <CommandFooter>
                            <div className="flex items-center gap-4">
                                <div className="flex items-center gap-2">
                                    <KbdGroup>
                                        <Kbd>
                                            <ArrowUpIcon />
                                        </Kbd>
                                        <Kbd>
                                            <ArrowDownIcon />
                                        </Kbd>
                                    </KbdGroup>
                                    <span>Navigate</span>
                                </div>
                                <div className="flex items-center gap-2">
                                    <Kbd>
                                        <CornerDownLeftIcon />
                                    </Kbd>
                                    <span>Open</span>
                                </div>
                            </div>
                            <div className="flex items-center gap-2">
                                <Kbd>Esc</Kbd>
                                <span>Close</span>
                            </div>
                        </CommandFooter>
                    </Command>
                </CommandDialogPopup>
            </CommandDialog>
            <StashChangesDialog
                repoId={stashRepoId ?? 0}
                open={stashRepoId !== null}
                onClose={() => setStashRepoId(null)}
            />
        </>
    );
}
