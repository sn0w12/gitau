import { LabelBadge } from "@/components/github/label-badge";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { Frame, FrameHeader, FramePanel } from "@/components/ui/frame";
import { Skeleton } from "@/components/ui/skeleton";
import type { GithubLabel, GithubUser } from "@/lib/backend/protocol";

function initials(login: string): string {
    return login.slice(0, 1).toUpperCase() || "?";
}

export function AvatarStack({ users }: { users: GithubUser[] }) {
    if (users.length === 0) {
        return <span className="text-sm text-muted-foreground">None</span>;
    }
    return (
        <div className="flex -space-x-[0.4rem]">
            {users.map((user) => (
                <Avatar key={user.login} className="size-6 ring-2 ring-card">
                    <AvatarImage src={user.avatarUrl || undefined} />
                    <AvatarFallback>{initials(user.login)}</AvatarFallback>
                </Avatar>
            ))}
        </div>
    );
}

/** One labelled block of the thread sidebar. `children` is the value. */
export function SidebarBlock({
    label,
    children,
}: {
    label: string;
    children: React.ReactNode;
}) {
    return (
        <FramePanel className="flex flex-col p-2 pt-1">
            <span className="ui-selectable">{label}</span>
            {children}
        </FramePanel>
    );
}

export function LabelList({ labels }: { labels: GithubLabel[] }) {
    if (labels.length === 0) {
        return <div className="text-sm text-muted-foreground">None</div>;
    }
    return (
        <div className="space-x-1">
            {labels.map((label) => (
                <LabelBadge key={label.name} label={label} />
            ))}
        </div>
    );
}

/** Avatar-stack placeholder, matching AvatarStack's overlap and ring. */
export function AvatarStackSkeleton({ count = 2 }: { count?: number }) {
    return (
        <div className="flex -space-x-[0.4rem]">
            {Array.from({ length: count }, (_, index) => (
                <Skeleton
                    key={index}
                    className="size-6 rounded-full ring-2 ring-card"
                />
            ))}
        </div>
    );
}

/** Blocks in the order PullSidebar renders them, so loading does not
 * rearrange the sidebar. */
export function PullSidebarSkeleton() {
    return (
        <Frame className="flex h-fit w-full flex-col">
            <SidebarBlock label="Changes">
                <div className="flex flex-col gap-1.5">
                    <Skeleton className="h-4 w-16" />
                    <div className="flex gap-1.5">
                        <Skeleton className="h-4 w-10" />
                        <Skeleton className="h-4 w-10" />
                    </div>
                </div>
            </SidebarBlock>
            <SidebarBlock label="Checks">
                <div className="flex flex-col gap-1.5">
                    <Skeleton className="h-4 w-4/5" />
                    <Skeleton className="h-4 w-3/5" />
                </div>
            </SidebarBlock>
            <SidebarBlock label="Reviewers">
                <AvatarStackSkeleton />
            </SidebarBlock>
            <SidebarBlock label="Labels">
                <div className="flex gap-1">
                    <Skeleton className="h-5 w-14 rounded-full" />
                    <Skeleton className="h-5 w-20 rounded-full" />
                </div>
            </SidebarBlock>
            <SidebarBlock label="Reviews">
                <div className="flex flex-col gap-1.5">
                    <Skeleton className="h-4 w-2/3" />
                    <Skeleton className="h-4 w-1/2" />
                </div>
            </SidebarBlock>
            <SidebarBlock label="Participants">
                <AvatarStackSkeleton />
            </SidebarBlock>
        </Frame>
    );
}

/** Blocks in the order IssueSidebar renders them. */
export function IssueSidebarSkeleton() {
    return (
        <Frame className="flex h-fit w-full flex-col">
            <SidebarBlock label="Assignees">
                <AvatarStackSkeleton />
            </SidebarBlock>
            <SidebarBlock label="Labels">
                <div className="flex gap-1">
                    <Skeleton className="h-5 w-14 rounded-full" />
                    <Skeleton className="h-5 w-20 rounded-full" />
                </div>
            </SidebarBlock>
            <SidebarBlock label="Participants">
                <AvatarStackSkeleton />
            </SidebarBlock>
        </Frame>
    );
}

/** Event-row placeholder mirroring TimelineEvent's two leading circles. */
export function EventSkeleton() {
    return (
        <>
            <div className="ml-4 h-1.5 w-0.5 bg-muted" />
            <div className="ui-selectable flex items-center gap-1 px-1.5 py-1">
                <Skeleton className="size-6 rounded-full" />
                <Skeleton className="size-6 rounded-full" />
                <Skeleton className="h-4 w-48" />
            </div>
            <div className="ml-4 h-1.5 w-0.5 bg-muted" />
        </>
    );
}

/** Message-shaped placeholder mirroring TimelineMessage's Frame layout. */
export function MessageSkeleton({ lines }: { lines: number }) {
    const widths = ["w-full", "w-11/12", "w-2/3"];
    return (
        <Frame className="ui-selectable">
            <FrameHeader className="flex flex-row items-center gap-1 px-2 py-1.5">
                <Skeleton className="size-6 rounded-full" />
                <Skeleton className="h-4 w-40" />
            </FrameHeader>
            <FramePanel className="px-3 py-2">
                <div className="flex flex-col gap-2">
                    {Array.from({ length: lines }, (_, index) => (
                        <Skeleton
                            key={index}
                            className={`h-4 ${widths[index % widths.length]}`}
                        />
                    ))}
                </div>
            </FramePanel>
        </Frame>
    );
}

/** Composer-shaped placeholder mirroring ConversationInput: a Write/Preview
 * tab list, nine toolbar buttons, the text area, then the action row. */
export function ComposerSkeleton() {
    return (
        <div className="w-full">
            <Frame>
                <FrameHeader className="flex flex-row justify-between px-2 py-0">
                    <div className="flex gap-3">
                        <Skeleton className="h-4 w-12" />
                        <Skeleton className="h-4 w-14" />
                    </div>
                    <div className="flex gap-1">
                        {Array.from({ length: 9 }, (_, index) => (
                            <Skeleton
                                key={index}
                                className="size-8 rounded-md"
                            />
                        ))}
                    </div>
                </FrameHeader>
                <FramePanel className="px-3 py-2">
                    <Skeleton className="h-20 w-full" />
                </FramePanel>
            </Frame>
            <div className="flex justify-end gap-1 pt-2">
                <Skeleton className="h-8 w-32 rounded-lg" />
                <Skeleton className="h-8 w-24 rounded-lg" />
            </div>
        </div>
    );
}
