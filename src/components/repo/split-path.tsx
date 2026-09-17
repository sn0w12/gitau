import { cn } from "@/lib/utils";

export function SplitPath({
    path,
    className,
}: {
    path: string;
    className?: string;
}) {
    const [before, filename] = path.match(/^(.*)\/([^/]*)$/)?.slice(1) ?? [
        "",
        path,
    ];
    const midpoint = Math.floor(filename.length / 2);
    const head = filename.length > 12 ? filename.slice(0, midpoint) : filename;
    const tail = filename.length > 12 ? filename.slice(midpoint) : "";

    return (
        <div className={cn("flex min-w-0 items-baseline", className)}>
            {before && (
                <span
                    className="min-w-0 truncate text-muted-foreground"
                    style={{ flexShrink: 100000 }}
                >
                    {before}
                </span>
            )}
            <span className="max-w-full min-w-0 truncate whitespace-nowrap">
                {before && <span className="text-muted-foreground">/</span>}
                <span className="font-semibold">{head}</span>
            </span>
            {tail && (
                <span
                    dir="rtl"
                    className="max-w-full min-w-0 truncate font-semibold whitespace-nowrap"
                >
                    <bdi>{tail}</bdi>
                </span>
            )}
        </div>
    );
}
