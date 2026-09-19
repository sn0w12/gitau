import type { Modifiers } from "@dnd-kit/abstract";
import { DragDropProvider } from "@dnd-kit/react";
import { isSortable, useSortable } from "@dnd-kit/react/sortable";
import { useSelector } from "@tanstack/react-store";
import {
    CircleDot,
    Download,
    FlaskConical,
    Folder,
    GitPullRequestArrow,
    Inbox,
    Plus,
    Settings,
    User,
} from "lucide-react";
import type { ComponentType, ReactNode } from "react";
import { useCallback, useEffect, useMemo, useState } from "react";

import { CloneRepoDialog } from "@/components/repo/dialogs/clone-dialog";
import { NewRepoDialog } from "@/components/repo/dialogs/new-repo-dialog";
import { RepoContextMenu } from "@/components/repo/repo-context-menu";
import { useAppCommands } from "@/contexts/app-command-context";
import { useGithubAccount } from "@/hooks/github/use-github-account";
import { useGithubUnreadCount } from "@/hooks/github/use-github-inbox";
import { useOpenRepository } from "@/hooks/repositories/use-open-repository";
import { useRepoAvatarByPath } from "@/hooks/repositories/use-repo-avatar";
import { useRemoteIcon } from "@/hooks/repositories/use-repository-queries";
import { useSettingValue } from "@/hooks/settings/use-setting";
import { useActiveTabRouter } from "@/hooks/tabs/use-active-tab-router";
import { addExistingRepositoryFromDisk } from "@/lib/repositories/add-repository";
import { toastError } from "@/lib/toast-error";
import {
    moveRepo,
    reorderPinnedRepos,
    repositoryStore,
    selectPinnedRepoEntries,
    selectUnpinnedRepoEntries,
} from "@/stores/repository-store";
import { setSetting } from "@/stores/settings-store";

import { RestrictToList } from "./dnd/restrict-to-list";
import { RepoLabel } from "./repo/repo-label";
import { Avatar, AvatarFallback, AvatarImage } from "./ui/avatar";
import { Badge } from "./ui/badge";
import type { ButtonProps } from "./ui/button";
import { ContextMenuShortcut } from "./ui/context-menu";
import { Menu, MenuGroup, MenuItem, MenuPopup, MenuTrigger } from "./ui/menu";
import { ScrollArea } from "./ui/scroll-area";
import {
    Sidebar,
    SidebarContent,
    SidebarFooter,
    SidebarGroup,
    SidebarMenuButton,
    SidebarMenuItem,
    SidebarSeparator,
} from "./ui/sidebar";
import {
    TooltipCreateHandle,
    TooltipPayloadHost,
    TooltipProvider,
    TooltipTrigger,
} from "./ui/tooltip";

const handle = TooltipCreateHandle<ComponentType>();
// Built at drag time: reads live bounds so resize/scroll stay correct.
const restrictToRepoList = (slot: string) =>
    RestrictToList.configure({
        getBounds: () =>
            document
                .querySelector(`[data-slot="${slot}"]`)
                ?.getBoundingClientRect() ?? null,
        axis: "y",
    });
const inboxPayload = () => {
    return <span>Inbox</span>;
};
const pullRequestsPayload = () => {
    return <span>Pull Requests</span>;
};
const issuesPayload = () => {
    return <span>Issues</span>;
};
const addPayload = () => {
    return <span>Add Repo</span>;
};
const accountPayload = () => {
    return <span>Account</span>;
};

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
    return (
        <Sidebar variant="inset" collapsible="icon" {...props}>
            <TooltipProvider>
                <SidebarContent className="gap-1">
                    <SidebarGroup className="gap-1">
                        <InboxSidebarItem />
                        <SidebarAction
                            icon={<GitPullRequestArrow />}
                            title="Pull Requests"
                            payload={pullRequestsPayload}
                        />
                        <SidebarAction
                            icon={<CircleDot />}
                            title="Issues"
                            payload={issuesPayload}
                        />
                    </SidebarGroup>
                    <SidebarSeparator className="data-[orientation=horizontal]:w-auto" />
                    <SidebarGroup className="min-h-0 flex-1 gap-1">
                        <ScrollArea
                            scrollBar={false}
                            fill
                            scrollFade
                            className="h-auto"
                        >
                            <RegisteredRepoButtons />
                        </ScrollArea>
                        <NewRepoButton />
                    </SidebarGroup>
                </SidebarContent>
                <SidebarFooter>
                    <SidebarSeparator className="mx-0 data-[orientation=horizontal]:w-auto" />
                    <AccountButton />
                </SidebarFooter>
                <TooltipPayloadHost handle={handle} side="right" />
            </TooltipProvider>
        </Sidebar>
    );
}

function SidebarAction({
    icon,
    title,
    payload,
    variant = "ghost",
}: {
    icon: ReactNode;
    title: string;
    payload: () => React.JSX.Element;
    variant?: ButtonProps["variant"];
}) {
    return (
        <SidebarMenuItem>
            <TooltipTrigger
                handle={handle}
                payload={payload}
                render={
                    <SidebarMenuButton variant={variant}>
                        {icon}
                        <span>{title}</span>
                    </SidebarMenuButton>
                }
            />
        </SidebarMenuItem>
    );
}

/**
 * Sidebar entry for the inbox: navigates to the page and overlays the
 * unread count from the shared notifications cache. The badge lives
 * outside the menu button so the button keeps exactly its icon and
 * label children; it stays hidden while signed out or fully read.
 */
function InboxSidebarItem() {
    const router = useActiveTabRouter();
    const { unread } = useGithubUnreadCount();
    return (
        <SidebarMenuItem>
            <TooltipTrigger
                handle={handle}
                payload={inboxPayload}
                render={
                    <SidebarMenuButton
                        variant="ghost"
                        onClick={() => void router?.navigate({ to: "/inbox" })}
                    >
                        <Inbox />
                        <span>Inbox</span>
                    </SidebarMenuButton>
                }
            />
            {unread > 0 ? (
                <Badge
                    size="sm"
                    variant="infoFull"
                    className="pointer-events-none absolute top-0 right-0"
                >
                    {unread > 99 ? "99+" : unread}
                </Badge>
            ) : null}
        </SidebarMenuItem>
    );
}

function RepoButton({
    repoPath,
    repoId: _repoId,
    index,
    modifiers,
}: {
    repoPath: string;
    repoId?: number;
    index: number;
    modifiers: Modifiers;
}) {
    const avatar = useRepoAvatarByPath(repoPath);
    const openRepo = useOpenRepository();
    const { ref } = useSortable({ id: repoPath, index, modifiers });
    const repoNamePayload = () => {
        return (
            <RepoLabel
                repo={
                    avatar.owner
                        ? `${avatar.owner}/${avatar.name}`
                        : avatar.name
                }
            />
        );
    };

    return (
        <SidebarMenuItem ref={ref}>
            <RepoContextMenu repoPath={repoPath}>
                <TooltipTrigger
                    handle={handle}
                    payload={repoNamePayload}
                    render={
                        <SidebarMenuButton
                            variant="secondary"
                            className="relative group-data-[collapsible=icon]:p-0!"
                            aria-label={avatar.name}
                            onClick={() => void openRepo(repoPath)}
                            render={
                                <Avatar>
                                    <AvatarImage
                                        src={avatar.icon?.dataUrl}
                                        alt={`${avatar.name} icon`}
                                    />
                                    <AvatarFallback
                                        className="block size-auto bg-transparent"
                                        render={<div />}
                                    >
                                        {avatar.initial}
                                    </AvatarFallback>
                                </Avatar>
                            }
                        />
                    }
                />
            </RepoContextMenu>
        </SidebarMenuItem>
    );
}

function RegisteredRepoButtons() {
    const pinnedRepos = useSettingValue("pinnedRepos");
    const pinned = useSelector(repositoryStore, (state) =>
        selectPinnedRepoEntries(state, pinnedRepos)
    );
    const unpinned = useSelector(repositoryStore, (state) =>
        selectUnpinnedRepoEntries(state, pinnedRepos)
    );
    const restrictPinnedToList = useMemo(
        () => restrictToRepoList("pinned-repo-list"),
        []
    );
    const restrictToList = useMemo(() => restrictToRepoList("repo-list"), []);

    return (
        <div className="flex flex-col gap-1">
            {pinned.length > 0 ? (
                <DragDropProvider
                    onDragEnd={(event) => {
                        if (event.canceled) return;
                        const { source } = event.operation;
                        if (
                            isSortable(source) &&
                            source.initialIndex !== source.index
                        ) {
                            const next = reorderPinnedRepos(
                                pinnedRepos,
                                repositoryStore.state,
                                String(source.id),
                                source.index
                            );
                            if (next) setSetting("pinnedRepos", next);
                        }
                    }}
                >
                    <div
                        className="flex flex-col gap-1"
                        data-slot="pinned-repo-list"
                    >
                        {pinned.map((entry, index) => (
                            <RepoButton
                                key={entry.path}
                                repoId={entry.repoId}
                                repoPath={entry.path}
                                index={index}
                                modifiers={[restrictPinnedToList]}
                            />
                        ))}
                    </div>
                </DragDropProvider>
            ) : null}
            {pinned.length > 0 && unpinned.length > 0 ? (
                <SidebarSeparator className="data-[orientation=horizontal]:w-auto" />
            ) : null}
            <DragDropProvider
                onDragEnd={(event) => {
                    if (event.canceled) return;
                    const { source } = event.operation;
                    if (
                        isSortable(source) &&
                        source.initialIndex !== source.index
                    ) {
                        moveRepo(String(source.id), source.index, pinnedRepos);
                    }
                }}
            >
                <div className="flex flex-col gap-1" data-slot="repo-list">
                    {unpinned.map((entry, index) => (
                        <RepoButton
                            key={entry.path}
                            repoId={entry.repoId}
                            repoPath={entry.path}
                            index={index}
                            modifiers={[restrictToList]}
                        />
                    ))}
                </div>
            </DragDropProvider>
        </div>
    );
}

function NewRepoButton() {
    const openRepo = useOpenRepository();
    const [dialog, setDialog] = useState<null | "new" | "clone">(null);
    const { register } = useAppCommands();

    const addExisting = useCallback(async () => {
        const outcome = await addExistingRepositoryFromDisk();
        switch (outcome.status) {
            case "cancelled":
                return;
            case "failed":
                toastError("Could not open repository", outcome.error);
                return;
            case "added":
                await openRepo(outcome.repo.repoPath);
        }
    }, [openRepo]);

    useEffect(() => {
        const disposeNew = register("newRepository", () => setDialog("new"));
        const disposeAdd = register("addRepository", addExisting);
        const disposeClone = register("cloneRepository", () =>
            setDialog("clone")
        );
        return () => {
            disposeNew();
            disposeAdd();
            disposeClone();
        };
    }, [addExisting, register]);

    return (
        <SidebarMenuItem>
            <Menu>
                <MenuTrigger>
                    <TooltipTrigger
                        handle={handle}
                        payload={addPayload}
                        render={
                            <SidebarMenuButton variant="secondary">
                                <Plus />
                                <span>Add Repo</span>
                            </SidebarMenuButton>
                        }
                    />
                </MenuTrigger>
                <MenuPopup side="right">
                    <MenuGroup>
                        <MenuItem onClick={() => setDialog("new")}>
                            <Plus />
                            New
                        </MenuItem>
                        <MenuItem onClick={() => void addExisting()}>
                            <Folder />
                            Existing
                        </MenuItem>
                        <MenuItem onClick={() => setDialog("clone")}>
                            <Download />
                            Clone
                        </MenuItem>
                    </MenuGroup>
                </MenuPopup>
            </Menu>
            <NewRepoDialog
                open={dialog === "new"}
                onClose={() => setDialog(null)}
                onCreated={(repoPath) => {
                    setDialog(null);
                    void openRepo(repoPath);
                }}
            />
            <CloneRepoDialog
                open={dialog === "clone"}
                onClose={() => setDialog(null)}
                onCloned={(repoPath) => {
                    setDialog(null);
                    void openRepo(repoPath);
                }}
            />
        </SidebarMenuItem>
    );
}

function AccountButton() {
    const router = useActiveTabRouter();
    const account = useGithubAccount();
    const profile = account.data;

    // Resolve through the icon pipeline so the avatar is a cached data URL
    // rather than a webview network load.
    const avatar = useRemoteIcon(
        profile ? `https://github.com/${profile.login}` : undefined
    );

    return (
        <SidebarMenuItem>
            <Menu>
                <MenuTrigger>
                    <TooltipTrigger
                        handle={handle}
                        payload={accountPayload}
                        render={
                            <SidebarMenuButton
                                variant="secondary"
                                aria-label="account"
                                className="relative group-data-[collapsible=icon]:p-0!"
                                render={
                                    <Avatar className="size-full">
                                        <AvatarImage
                                            src={avatar.data?.dataUrl}
                                        />
                                        <AvatarFallback>
                                            <User />
                                        </AvatarFallback>
                                    </Avatar>
                                }
                            />
                        }
                    />
                </MenuTrigger>
                <MenuPopup side="right" align="end">
                    <MenuGroup>
                        <MenuItem
                            onClick={() =>
                                void router?.navigate({ to: "/account" })
                            }
                        >
                            <User />
                            Account
                        </MenuItem>
                        <MenuItem
                            onClick={() =>
                                void router?.navigate({ to: "/settings" })
                            }
                        >
                            <Settings />
                            Settings
                            <ContextMenuShortcut shortcut="goToSettings" />
                        </MenuItem>
                        {import.meta.env.DEV ? (
                            <MenuItem
                                onClick={() =>
                                    void router?.navigate({ to: "/dev" })
                                }
                            >
                                <FlaskConical />
                                Dev playground
                            </MenuItem>
                        ) : null}
                    </MenuGroup>
                </MenuPopup>
            </Menu>
        </SidebarMenuItem>
    );
}
