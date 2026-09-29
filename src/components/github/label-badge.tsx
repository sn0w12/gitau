import { Badge } from "@/components/ui/badge";
import type { GithubLabel } from "@/lib/backend/protocol";
import { getTextColor } from "@/lib/utils";

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
