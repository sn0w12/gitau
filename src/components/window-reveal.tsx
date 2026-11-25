import { useEffect } from "react";

import { reportLaunchReady } from "@/lib/bootstrap/launch-report";
import { revealWindow } from "@/lib/bootstrap/window-reveal";

/**
 * Renders nothing. Revealing belongs to the render lifecycle, not to a timer:
 * mounting this inside a tree reveals the window once that tree has committed,
 * so the frame the user sees first is the rendered app, never the empty
 * document.
 */
export function WindowReveal() {
    useEffect(() => {
        revealWindow();
        reportLaunchReady();
    }, []);

    return null;
}
