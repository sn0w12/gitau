"use no memo";

import { DragDropProvider } from "@dnd-kit/react";
import { isSortable, useSortable } from "@dnd-kit/react/sortable";
import { useSelector } from "@tanstack/react-store";
import { GitFork, Star } from "lucide-react";

import { RepoContextMenu } from "@/components/repo/repo-context-menu";
import { RepoLabel } from "@/components/repo/repo-label";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Frame, FrameFooter, FramePanel } from "@/components/ui/frame";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
    Tooltip,
    TooltipContent,
    TooltipTrigger,
} from "@/components/ui/tooltip";
import { useOpenRepository } from "@/hooks/repositories/use-open-repository";
import { useRepoAvatarByPath } from "@/hooks/repositories/use-repo-avatar";
import { useRemoteRepoInfoByPath } from "@/hooks/repositories/use-repository-queries";
import { useSettingValue } from "@/hooks/settings/use-setting";
import type { LanguageShare } from "@/lib/backend/protocol";
import type { RepositoryEntry } from "@/stores/repository-store";
import {
    moveRepo,
    reorderPinnedRepos,
    repositoryStore,
    selectPinnedRepoEntries,
    selectUnpinnedRepoEntries,
} from "@/stores/repository-store";
import { setSetting } from "@/stores/settings-store";

export function HomePage() {
    const pinnedRepos = useSettingValue("pinnedRepos");
    const pinned = useSelector(repositoryStore, (state) =>
        selectPinnedRepoEntries(state, pinnedRepos)
    );
    const unpinned = useSelector(repositoryStore, (state) =>
        selectUnpinnedRepoEntries(state, pinnedRepos)
    );

    return (
        <ScrollArea className="size-full">
            <div className="flex size-full p-2">
                <div className="w-full space-y-2">
                    <h1 className="ui-selectable w-full pt-6 pb-2 text-center font-heading text-8xl font-semibold tracking-tight">
                        GITAU
                    </h1>
                    <div className="grid grid-cols-1 gap-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
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
                                        if (next)
                                            setSetting("pinnedRepos", next);
                                    }
                                }}
                            >
                                {pinned.map((entry, index) => (
                                    <RepoCard
                                        key={entry.path}
                                        repo={entry}
                                        index={index}
                                    />
                                ))}
                            </DragDropProvider>
                        ) : null}
                        <DragDropProvider
                            onDragEnd={(event) => {
                                if (event.canceled) return;
                                const { source } = event.operation;
                                if (
                                    isSortable(source) &&
                                    source.initialIndex !== source.index
                                ) {
                                    moveRepo(
                                        String(source.id),
                                        source.index,
                                        pinnedRepos
                                    );
                                }
                            }}
                        >
                            {unpinned.map((entry, index) => (
                                <RepoCard
                                    key={entry.path}
                                    repo={entry}
                                    index={index}
                                />
                            ))}
                        </DragDropProvider>
                    </div>
                </div>
            </div>
        </ScrollArea>
    );
}

function RepoCard({ repo, index }: { repo: RepositoryEntry; index: number }) {
    const avatar = useRepoAvatarByPath(repo.path);
    const openRepo = useOpenRepository();
    const infoQuery = useRemoteRepoInfoByPath(repo.path);
    const { ref } = useSortable({ id: repo.path, index });
    const info = infoQuery.data ?? {};
    const description = info?.description;
    const stars = info?.stars ?? undefined;
    const forks = info?.forks ?? undefined;
    const languages = info?.languages ?? [];

    return (
        <RepoContextMenu repoPath={repo.path} side="bottom">
            <Frame
                ref={ref}
                className="group grid cursor-pointer grid-rows-[1fr_auto] text-sm"
                onClick={() => {
                    void openRepo(repo.path);
                }}
            >
                <FramePanel className="p-3">
                    <div className="flex items-center gap-1">
                        <Avatar className="size-6 rounded-lg">
                            <AvatarImage
                                src={avatar.icon?.dataUrl}
                                alt={`${avatar.name} icon`}
                            />
                            <AvatarFallback>{avatar.initial}</AvatarFallback>
                        </Avatar>
                        <RepoLabel
                            className="gap-0 group-hover:underline"
                            repo={
                                avatar.owner
                                    ? `${avatar.owner}/${avatar.name}`
                                    : avatar.name
                            }
                        />
                    </div>
                    {description && (
                        <p className="line-clamp-2 pt-0.5">{description}</p>
                    )}
                </FramePanel>
                {(stars !== undefined ||
                    forks !== undefined ||
                    languages.length > 0) && (
                    <FrameFooter className="flex items-center gap-1.5 px-2 py-1 font-mono [&_svg:not([class*='size-'])]:size-4">
                        {stars !== undefined && (
                            <span className="flex items-center gap-0.5">
                                <Star />
                                {stars}
                            </span>
                        )}
                        {forks !== undefined && (
                            <span className="flex items-center gap-0.5">
                                <GitFork />
                                {forks}
                            </span>
                        )}
                        {languages.length > 0 && (
                            <LanguageBar items={languages} />
                        )}
                    </FrameFooter>
                )}
            </Frame>
        </RepoContextMenu>
    );
}

function LanguageBar({ items }: { items: LanguageShare[] }) {
    return (
        <div className="ml-auto flex max-w-52 min-w-0 flex-1 flex-col gap-1">
            <Tooltip>
                <TooltipTrigger delay={300}>
                    <div className="flex h-1.5 gap-px overflow-hidden rounded-full">
                        {items.map((item) => (
                            <span
                                key={item.language}
                                className="h-full"
                                style={{
                                    width: `${item.percent}%`,
                                    backgroundColor:
                                        item.color || "var(--muted-foreground)",
                                }}
                            />
                        ))}
                    </div>
                </TooltipTrigger>
                <TooltipContent className="flex gap-1">
                    {items.map((item) => (
                        <span
                            key={item.language}
                            className="flex items-center justify-between gap-4"
                        >
                            <div className="flex items-center gap-1">
                                <div
                                    className="size-2 rounded-full"
                                    style={{
                                        backgroundColor:
                                            item.color ||
                                            "var(--muted-foreground)",
                                    }}
                                />
                                {item.language}
                            </div>
                            <span className="font-mono">{item.percent}%</span>
                        </span>
                    ))}
                </TooltipContent>
            </Tooltip>
        </div>
    );
}
