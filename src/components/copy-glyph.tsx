import { Check, Copy } from "lucide-react";

import { Swap } from "./ui/swap";

/** Copy icon that cross-fades to a checkmark, so a shape change reads as one
 *  icon morphing rather than two glyphs swapping. */
export function CopyGlyph({ copied }: { copied: boolean }) {
    return (
        <Swap
            when={copied}
            on={<Check className="size-4" />}
            off={<Copy className="size-4" />}
        />
    );
}
