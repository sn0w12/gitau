import { File, Tag } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";

import { CommitContextMenu } from "@/components/repo/history/commit-context-menu";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { useTabId } from "@/contexts/tab-context";
import { useCommitDiffPrefetch } from "@/hooks/changes/use-diff-prefetch";
import { useInfiniteHistoryPage } from "@/hooks/repositories/use-repository-queries";
import type { CommitSummary, SearchMatchRange } from "@/lib/backend/protocol";
import { formatRelativeDate } from "@/lib/utils";

import { Input } from "../../ui/input";
import { Tooltip, TooltipContent, TooltipTrigger } from "../../ui/tooltip";

const LOAD_MORE_THRESHOLD_PX = 400;

const SEARCH_DEBOUNCE_MS = 250;

const HISTORY_ROW_HEIGHT_PX = 50;
const HISTORY_OVERSCAN_ROWS = 4;

export function HistoryPanel({
    repoId,
    selectedId,
    onSelect,
    active = true,
}: {
    repoId: number;
    selectedId: string | null;
    onSelect: (commitId: string | null) => void;
    active?: boolean;
}) {
    const [searchInput, setSearchInput] = useState("");
    const [search, setSearch] = useState("");
    const history = useInfiniteHistoryPage(repoId, search);
    const tabId = useTabId();
    const { warm, cancelWarm } = useCommitDiffPrefetch(repoId, tabId);

    // Debounce the search term so keystrokes do not re-query the backend on
    // every one; the query key carries the trimmed term.
    useEffect(() => {
        const timer = setTimeout(
            () => setSearch(searchInput.trim()),
            SEARCH_DEBOUNCE_MS
        );
        return () => clearTimeout(timer);
    }, [searchInput]);

    const containerRef = useRef<HTMLDivElement | null>(null);
    const commits = useMemo(
        () => history.data?.pages.flatMap((page) => page.commits) ?? [],
        [history.data]
    );
    const { hasNextPage, isFetchingNextPage, isPending, fetchNextPage } =
        history;
    const [scrollTop, setScrollTop] = useState(0);
    const [viewportHeight, setViewportHeight] = useState(0);

    useEffect(() => {
        if (!active || isPending) return;
        const viewport = containerRef.current?.querySelector<HTMLElement>(
            '[data-slot="scroll-area-viewport"]'
        );
        if (!viewport) return;
        const onScroll = () => setScrollTop(viewport.scrollTop);
        setViewportHeight(viewport.clientHeight);
        setScrollTop(viewport.scrollTop);
        viewport.addEventListener("scroll", onScroll);
        if (typeof ResizeObserver === "undefined") {
            return () => viewport.removeEventListener("scroll", onScroll);
        }
        const observer = new ResizeObserver(() => {
            setScrollTop(viewport.scrollTop);
            setViewportHeight(viewport.clientHeight);
        });
        observer.observe(viewport);
        return () => {
            viewport.removeEventListener("scroll", onScroll);
            observer.disconnect();
        };
    }, [active, isPending]);

    const firstVisible = Math.max(
        0,
        Math.floor(scrollTop / HISTORY_ROW_HEIGHT_PX) - HISTORY_OVERSCAN_ROWS
    );
    const visibleCount =
        Math.ceil(
            (viewportHeight || HISTORY_ROW_HEIGHT_PX * 10) /
                HISTORY_ROW_HEIGHT_PX
        ) +
        HISTORY_OVERSCAN_ROWS * 2;
    const lastVisible = Math.min(commits.length, firstVisible + visibleCount);
    const windowCommits = useMemo(
        () => commits.slice(firstVisible, lastVisible),
        [commits, firstVisible, lastVisible]
    );

    // Re-binds whenever guard values change so it never reads stale
    // hasNextPage/isFetching; the initial call auto-fills a viewport shorter
    // than the first page.
    useEffect(() => {
        const container = containerRef.current;
        if (!container || !active || isPending) return;
        const viewport = container.querySelector<HTMLElement>(
            '[data-slot="scroll-area-viewport"]'
        );
        if (!viewport) return;

        const maybeLoadMore = () => {
            if (!hasNextPage || isFetchingNextPage) return;
            // A hidden viewport measures 0x0 and would read as "at bottom".
            if (viewport.clientHeight === 0 && viewport.scrollHeight === 0) {
                return;
            }
            const distanceToBottom =
                viewport.scrollHeight -
                viewport.scrollTop -
                viewport.clientHeight;
            if (distanceToBottom < LOAD_MORE_THRESHOLD_PX) {
                void fetchNextPage();
            }
        };

        viewport.addEventListener("scroll", maybeLoadMore);
        maybeLoadMore();
        return () => viewport.removeEventListener("scroll", maybeLoadMore);
    }, [active, isPending, hasNextPage, isFetchingNextPage, fetchNextPage]);

    if (history.isPending) {
        return (
            <div className="divide-y">
                {Array.from({ length: 40 }).map((_, index) => (
                    <Skeleton
                        key={index}
                        className="h-10 w-full rounded-none"
                    />
                ))}
            </div>
        );
    }
    if (history.error && commits.length === 0) {
        return <HistoryError message={history.error.message} />;
    }

    if (commits.length === 0) {
        return (
            <p
                className="p-4 text-center text-xs text-muted-foreground"
                data-slot="history-empty"
            >
                {history.isFetched && search
                    ? "No commits match"
                    : "No commits yet"}
            </p>
        );
    }

    return (
        <div ref={containerRef} className="flex size-full min-h-0 flex-col">
            <div className="border-b px-0.5 pb-0.5">
                <Input
                    placeholder="Filter history..."
                    aria-label="Filter history"
                    value={searchInput}
                    onChange={(e) => setSearchInput(e.target.value)}
                />
            </div>
            <ScrollArea scrollFade data-slot="history-list" render={<ul />}>
                <div
                    className="relative"
                    style={{
                        height: Math.max(
                            commits.length * HISTORY_ROW_HEIGHT_PX,
                            viewportHeight
                        ),
                    }}
                >
                    {windowCommits.map((commit, offset) => (
                        <HistoryRow
                            key={commit.id}
                            repoId={repoId}
                            commit={commit}
                            selected={selectedId === commit.id}
                            top={
                                (firstVisible + offset) * HISTORY_ROW_HEIGHT_PX
                            }
                            onSelect={onSelect}
                            warm={warm}
                            cancelWarm={cancelWarm}
                        />
                    ))}
                </div>
                <HistoryFooter
                    fetching={history.isFetchingNextPage}
                    error={history.error?.message}
                    onRetry={() => void history.fetchNextPage()}
                />
            </ScrollArea>
        </div>
    );
}

/**
 * One virtualized commit row. Kept as its own component so row elements stay
 * stable when the panel re-renders for reasons that do not move the window
 * (context changes, selection, an unrelated state update), letting React skip
 * the row and its context menu. `top` positions the absolutely placed row.
 */
function HistoryRow({
    repoId,
    commit,
    selected,
    top,
    onSelect,
    warm,
    cancelWarm,
}: {
    repoId: number;
    commit: CommitSummary;
    selected: boolean;
    top: number;
    onSelect: (commitId: string | null) => void;
    warm: (commitId: string) => void;
    cancelWarm: () => void;
}) {
    return (
        <CommitContextMenu
            repoId={repoId}
            commit={commit}
            render={
                <li
                    role="button"
                    tabIndex={0}
                    aria-selected={selected || undefined}
                    data-selected={selected || undefined}
                    onClick={() => onSelect(selected ? null : commit.id)}
                    onMouseEnter={() => warm(commit.id)}
                    onMouseLeave={cancelWarm}
                    onFocus={() => warm(commit.id)}
                    onBlur={cancelWarm}
                    onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                            onSelect(selected ? null : commit.id);
                        }
                    }}
                    className="absolute right-0 left-0 cursor-pointer overflow-hidden px-3 py-1.5 hover:bg-accent data-selected:bg-accent/64"
                    style={{
                        top,
                        height: HISTORY_ROW_HEIGHT_PX,
                    }}
                >
                    <div className="flex w-full items-center gap-1.5">
                        <span className="truncate text-sm font-medium">
                            <HighlightedText
                                text={commit.summaryLine}
                                ranges={commit.matchRanges}
                                field="summary"
                            />
                        </span>
                        <span className="ml-auto shrink-0 font-mono text-xs text-muted-foreground">
                            #{commit.id.slice(0, 7)}
                        </span>
                    </div>
                    <div className="mt-0.5 flex items-center justify-between gap-x-1 gap-y-0.5 text-xs text-muted-foreground">
                        <div className="flex items-center gap-0.5">
                            <span>
                                <HighlightedText
                                    text={commit.author.name}
                                    ranges={commit.matchRanges}
                                    field="author"
                                />
                            </span>
                            <span className="hidden lg:inline">●</span>
                            <span className="hidden lg:inline">
                                {formatRelativeDate(
                                    commit.committer.timeSeconds * 1000
                                )}
                            </span>
                        </div>
                        <div className="flex items-center gap-0.5 font-mono">
                            {commit.tags.length > 0 && (
                                <Tooltip>
                                    <TooltipTrigger delay={100}>
                                        <span className="flex items-center gap-0.5">
                                            <Tag className="size-3" />
                                            {commit.tags.length}
                                        </span>
                                    </TooltipTrigger>
                                    <TooltipContent>
                                        {commit.tags.join(", ")}
                                    </TooltipContent>
                                    <span>/</span>
                                </Tooltip>
                            )}
                            <span className="flex items-center gap-0.5">
                                <File className="size-3" />
                                {commit.filesChanged}
                            </span>
                            <span>/</span>
                            <span className="text-success">
                                +{commit.additions}
                            </span>
                            <span>/</span>
                            <span className="text-destructive">
                                −{commit.deletions}
                            </span>
                        </div>
                    </div>
                </li>
            }
        />
    );
}

function HighlightedText({
    text,
    ranges,
    field,
}: {
    text: string;
    ranges: SearchMatchRange[] | undefined;
    field: "summary" | "author";
}) {
    const segments = useMemo(() => {
        if (!ranges || ranges.length === 0) return null;
        const fieldRanges = ranges
            .filter(
                (range) =>
                    range.field === field &&
                    range.start >= 0 &&
                    range.length > 0 &&
                    range.start < text.length
            )
            .map((range) => ({
                start: range.start,
                end: Math.min(text.length, range.start + range.length),
            }))
            .sort((a, b) => a.start - b.start);
        if (fieldRanges.length === 0) return null;
        const merged: { start: number; end: number }[] = [];
        for (const range of fieldRanges) {
            const last = merged[merged.length - 1];
            if (last && range.start <= last.end) {
                last.end = Math.max(last.end, range.end);
            } else {
                merged.push({ ...range });
            }
        }
        return merged;
    }, [ranges, field, text.length]);

    if (!segments) return text;
    const parts: ReactNode[] = [];
    let cursor = 0;
    for (const [index, segment] of segments.entries()) {
        if (segment.start > cursor) {
            parts.push(
                <span key={`t-${index}`}>
                    {text.slice(cursor, segment.start)}
                </span>
            );
        }
        parts.push(
            <mark
                key={`m-${index}`}
                className="bg-transparent font-bold text-warning"
            >
                {text.slice(segment.start, segment.end)}
            </mark>
        );
        cursor = segment.end;
    }
    if (cursor < text.length) {
        parts.push(<span key="t-rest">{text.slice(cursor)}</span>);
    }
    return parts;
}

function HistoryFooter({
    fetching,
    error,
    onRetry,
}: {
    fetching: boolean;
    error: string | undefined;
    onRetry: () => void;
}) {
    if (error) {
        return (
            <div className="flex flex-col items-center gap-1 p-3 text-xs text-destructive">
                <span>{error}</span>
                <Button size="xs" variant="outline" onClick={onRetry}>
                    Retry
                </Button>
            </div>
        );
    }
    if (fetching) {
        return (
            <div
                className="flex items-center justify-center p-3"
                data-slot="history-loading-more"
            >
                <Spinner className="size-4 text-muted-foreground" />
            </div>
        );
    }
    return null;
}

function HistoryError({ message }: { message: string }) {
    return <p className="p-2 text-xs text-destructive">{message}</p>;
}
