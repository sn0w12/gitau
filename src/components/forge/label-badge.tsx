import { Badge } from "@/components/ui/badge";
import type { Label } from "@/lib/backend/protocol";
import { getTextColor } from "@/lib/utils";

export function LabelBadge({ label }: { label: Label }) {
    return (
        <Badge
            className={
                getTextColor(label.color) === "bright"
                    ? "text-light"
                    : "text-dark"
            }
            style={{
                backgroundColor: `#${label.color}`,
            }}
        >
            {label.name}
        </Badge>
    );
}
