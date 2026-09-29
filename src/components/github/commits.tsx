import { GitCommitHorizontal } from "lucide-react";

import { MessageSpacer } from "@/components/github/message";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import type { GithubPullRequestCommit } from "@/lib/backend/protocol";

/** The subject line carries the intent of the commit; the body only matters
 * on the commit page. */
function subject(message: string): string {
    return message.split("\n")[0].trim();
}

/** One commit in the pull request timeline, laid out like the event rows
 * around it. */
export function TimelineCommit({
    commit,
    last,
}: {
    commit: GithubPullRequestCommit;
    last?: boolean;
}) {
    const author = commit.author.login;
    return (
        <>
            <MessageSpacer />
            <div className="ui-selectable flex items-center gap-1 px-1.5 pt-1 pb-0.5 text-sm">
                <div className="ml-0.5 flex size-5.5 items-center justify-center rounded-full bg-primary text-background">
                    <GitCommitHorizontal className="size-4" />
                </div>
                <Avatar className="size-6.5">
                    <AvatarImage src={commit.author.avatarUrl || undefined} />
                    <AvatarFallback>
                        {author.slice(0, 1).toUpperCase() || "?"}
                    </AvatarFallback>
                </Avatar>
                <span className="min-w-0 flex-1 truncate">
                    {subject(commit.message)}
                </span>
                <span className="shrink-0 font-mono text-xs text-muted-foreground">
                    {commit.sha.slice(0, 7)}
                </span>
            </div>
            {!last && <MessageSpacer />}
        </>
    );
}
