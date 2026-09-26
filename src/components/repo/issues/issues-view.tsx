import { CircleDot } from "lucide-react";
import { useMemo, useState } from "react";

import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
    Empty,
    EmptyDescription,
    EmptyHeader,
    EmptyMedia,
    EmptyTitle,
} from "@/components/ui/empty";
import { Frame } from "@/components/ui/frame";
import {
    Select,
    SelectItem,
    SelectPopup,
    SelectTrigger,
    SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import {
    Table,
    TableBody,
    TableCell,
    TableHead,
    TableHeader,
    TableRow,
} from "@/components/ui/table";
import { Tabs, TabsList, TabsPanel, TabsTab } from "@/components/ui/tabs";
import { useGithubAccount } from "@/hooks/github/use-github-account";
import {
    useGithubCoords,
    useRepoIssues,
} from "@/hooks/github/use-github-issues";
import { useActiveTabRouter } from "@/hooks/tabs/use-active-tab-router";
import type { GithubIssueListItem, GithubLabel } from "@/lib/backend/protocol";
import { getTextColor } from "@/lib/utils";

import { issueStatusOf, StatusBadge } from "../../issues/status-badge";

type TabState = "open" | "closed";

export function LabelBadge({ label }: { label: GithubLabel }) {
    return (
        <Badge
            className={
                getTextColor(label.color) === "bright"
                    ? "text-background dark:text-foreground"
                    : "text-foreground dark:text-background"
            }
            style={{
                backgroundColor: `#${label.color}`,
            }}
        >
            {label.name}
        </Badge>
    );
}

function AssigneeStack({
    assignees,
}: {
    assignees: GithubIssueListItem["assignees"];
}) {
    if (assignees.length === 0) return null;
    return (
        <div className="flex -space-x-[0.4rem]">
            {assignees.map((assignee) => (
                <Avatar
                    key={assignee.login}
                    className="size-5 ring-2 ring-card"
                >
                    <AvatarImage src={assignee.avatarUrl || undefined} />
                    <AvatarFallback>
                        {assignee.login.slice(0, 1).toUpperCase()}
                    </AvatarFallback>
                </Avatar>
            ))}
        </div>
    );
}

export function IssuesView({ repoId }: { repoId: number }) {
    const [tab, setTab] = useState<TabState>("open");
    const [label, setLabel] = useState<string>("none");

    const account = useGithubAccount();
    const { coords, isLoading: coordsLoading } = useGithubCoords(repoId);
    const issues = useRepoIssues(repoId, tab);

    const labels = useMemo(() => {
        const seen = new Map<string, GithubLabel>();
        for (const issue of issues.data ?? []) {
            for (const item of issue.labels) {
                if (!seen.has(item.name)) seen.set(item.name, item);
            }
        }
        return [...seen.values()].sort((a, b) => a.name.localeCompare(b.name));
    }, [issues.data]);

    const rows = useMemo(
        () =>
            (issues.data ?? []).filter(
                (issue) =>
                    label === "none" ||
                    issue.labels.some((item) => item.name === label)
            ),
        [issues.data, label]
    );

    return (
        <div className="flex h-full min-h-0 flex-col p-0.5">
            <Tabs
                className="min-h-0 flex-1 gap-0.5"
                value={tab}
                onValueChange={(value) => setTab(value as TabState)}
            >
                <div className="flex justify-between">
                    <TabsList>
                        <TabsTab value="open">Open</TabsTab>
                        <TabsTab value="closed">Closed</TabsTab>
                    </TabsList>
                    <div className="flex gap-1">
                        <Select
                            aria-label="Select label"
                            value={label}
                            onValueChange={(next) => {
                                if (typeof next === "string") setLabel(next);
                            }}
                            items={[
                                { value: "none", label: "None" },
                                ...labels.map((item) => ({
                                    value: item.name,
                                    label: item.name,
                                })),
                            ]}
                        >
                            <SelectTrigger>
                                <SelectValue />
                            </SelectTrigger>
                            <SelectPopup>
                                <SelectItem value="none">
                                    <span className="flex items-center gap-2">
                                        <span
                                            aria-hidden="true"
                                            className="size-1.5 rounded-full bg-muted-foreground"
                                        />
                                        <span className="truncate">None</span>
                                    </span>
                                </SelectItem>
                                {labels.map((item) => (
                                    <SelectItem
                                        key={item.name}
                                        value={item.name}
                                    >
                                        <span className="flex items-center gap-2">
                                            <LabelBadge label={item} />
                                        </span>
                                    </SelectItem>
                                ))}
                            </SelectPopup>
                        </Select>
                    </div>
                </div>
                <Frame className="w-full">
                    <TabsPanel
                        className="flex min-h-0 flex-1 flex-col"
                        value="open"
                    >
                        <IssuesTable
                            repoId={repoId}
                            state={tab}
                            rows={rows}
                            isLoading={
                                account.isLoading ||
                                coordsLoading ||
                                issues.isLoading
                            }
                            isError={issues.isError}
                            errorMessage={
                                issues.error instanceof Error
                                    ? issues.error.message
                                    : "Could not load issues"
                            }
                            onRetry={() => void issues.refetch()}
                            hasCoords={coords !== null}
                            signedIn={account.data != null}
                        />
                    </TabsPanel>
                    <TabsPanel
                        className="flex min-h-0 flex-1 flex-col"
                        value="closed"
                    >
                        <IssuesTable
                            repoId={repoId}
                            state={tab}
                            rows={rows}
                            isLoading={
                                account.isLoading ||
                                coordsLoading ||
                                issues.isLoading
                            }
                            isError={issues.isError}
                            errorMessage={
                                issues.error instanceof Error
                                    ? issues.error.message
                                    : "Could not load issues"
                            }
                            onRetry={() => void issues.refetch()}
                            hasCoords={coords !== null}
                            signedIn={account.data != null}
                        />
                    </TabsPanel>
                </Frame>
            </Tabs>
        </div>
    );
}

function IssuesTableHead() {
    return (
        <TableHeader>
            <TableRow>
                <TableHead className="h-9">Title</TableHead>
                <TableHead className="h-9">Status</TableHead>
                <TableHead className="h-9">Labels</TableHead>
                <TableHead className="h-9">Comments</TableHead>
                <TableHead className="h-9 text-right">Assignees</TableHead>
            </TableRow>
        </TableHeader>
    );
}

function IssuesTable({
    repoId,
    state,
    rows,
    isLoading,
    isError,
    errorMessage,
    onRetry,
    hasCoords,
    signedIn,
}: {
    repoId: number;
    state: TabState;
    rows: GithubIssueListItem[];
    isLoading: boolean;
    isError: boolean;
    errorMessage: string;
    onRetry: () => void;
    hasCoords: boolean;
    signedIn: boolean;
}) {
    const router = useActiveTabRouter();

    if (isLoading) {
        return <IssuesTableSkeleton />;
    }

    if (!signedIn) {
        return (
            <Empty>
                <EmptyMedia variant="icon">
                    <CircleDot />
                </EmptyMedia>
                <EmptyHeader>
                    <EmptyTitle>Connect GitHub</EmptyTitle>
                    <EmptyDescription>
                        Sign in on the Account page to read issues.
                    </EmptyDescription>
                </EmptyHeader>
            </Empty>
        );
    }

    if (!hasCoords) {
        return (
            <Empty>
                <EmptyMedia variant="icon">
                    <CircleDot />
                </EmptyMedia>
                <EmptyHeader>
                    <EmptyTitle>No GitHub remote</EmptyTitle>
                    <EmptyDescription>
                        This repository has no github.com remote, so there are
                        no issues to show.
                    </EmptyDescription>
                </EmptyHeader>
            </Empty>
        );
    }

    if (isError) {
        return (
            <Empty>
                <EmptyHeader>
                    <EmptyTitle>Could not load issues</EmptyTitle>
                    <EmptyDescription>{errorMessage}</EmptyDescription>
                </EmptyHeader>
                <Button variant="outline" size="sm" onClick={onRetry}>
                    Retry
                </Button>
            </Empty>
        );
    }

    if (rows.length === 0) {
        return (
            <Empty>
                <EmptyMedia variant="icon" className="mb-0">
                    <CircleDot />
                </EmptyMedia>
                <EmptyHeader>
                    <EmptyTitle>
                        {state === "open"
                            ? "No open issues"
                            : "No closed issues"}
                    </EmptyTitle>
                </EmptyHeader>
            </Empty>
        );
    }

    return (
        <Table containerClassName="min-h-0 flex-1 px-0.5" variant="card">
            <IssuesTableHead />
            <TableBody>
                {rows.map((issue) => (
                    <TableRow
                        key={issue.number}
                        className="cursor-pointer"
                        onClick={() => {
                            router?.navigate({
                                to: `/repo/${repoId}/issue/${issue.number}`,
                            });
                        }}
                    >
                        <TableCell className="font-medium">
                            {issue.title}
                        </TableCell>
                        <TableCell>
                            <StatusBadge status={issueStatusOf(issue.state)} />
                        </TableCell>
                        <TableCell className="space-x-1">
                            {issue.labels.map((label) => (
                                <LabelBadge key={label.name} label={label} />
                            ))}
                        </TableCell>
                        <TableCell className="font-mono">
                            {issue.commentCount}
                        </TableCell>
                        <TableCell>
                            <div className="flex min-h-5 items-center justify-end">
                                <AssigneeStack assignees={issue.assignees} />
                            </div>
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}

/** Same table shell with skeleton rows shaped like real rows. */
function IssuesTableSkeleton() {
    return (
        <Table containerClassName="min-h-0 flex-1" variant="card">
            <IssuesTableHead />
            <TableBody>
                {Array.from({ length: 8 }, (_, index) => (
                    <TableRow key={index}>
                        <TableCell className="font-medium">
                            <Skeleton
                                className="h-4"
                                style={{ width: `${62 - (index % 3) * 12}%` }}
                            />
                        </TableCell>
                        <TableCell>
                            <Skeleton className="h-5 w-16 rounded-full" />
                        </TableCell>
                        <TableCell>
                            <div className="flex gap-1">
                                <Skeleton className="h-5 w-14 rounded-full" />
                                {index % 2 === 0 ? (
                                    <Skeleton className="h-5 w-20 rounded-full" />
                                ) : null}
                            </div>
                        </TableCell>
                        <TableCell className="font-mono">
                            <Skeleton className="h-4 w-6" />
                        </TableCell>
                        <TableCell>
                            <div className="flex min-h-5 items-center justify-end">
                                <div className="flex -space-x-[0.4rem]">
                                    <Skeleton className="size-5 rounded-full ring-2 ring-card" />
                                    {index % 2 === 0 ? (
                                        <Skeleton className="size-5 rounded-full ring-2 ring-card" />
                                    ) : null}
                                </div>
                            </div>
                        </TableCell>
                    </TableRow>
                ))}
            </TableBody>
        </Table>
    );
}
