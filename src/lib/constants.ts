export const BORDER_GRADIENT =
    "relative after:absolute after:top-0 after:right-0 after:h-full after:w-px after:bg-gradient-to-t after:from-border after:via-border/28 after:to-transparent";

// Layout of a trigger's content only. `ToolbarTriggerFrame` owns the slot width
// and the chrome that has to track it: the bottom border, the hover fill, the
// divider and the focus ring. `w-full` rather than `w-auto` because a `button`
// shrink-wraps to its content even at `display: flex`, which would leave
// `justify-between` no free space and park the chevron or the counts next to
// the label.
export const REPO_TOOLBAR_TRIGGER_CLASS =
    "flex h-full w-full items-center justify-between px-4 py-2";
