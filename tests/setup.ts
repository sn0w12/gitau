// Shared Vitest setup. Runs once per test file, in every environment
// (node and jsdom).

(
    globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }
).IS_REACT_ACT_ENVIRONMENT = true;

// jsdom does not implement the Web Animations API. Base UI's ScrollArea
// calls viewport.getAnimations({ subtree: true }) from a deferred callback
// without any guard, so the TypeError escapes after the test finished and
// lands as an unhandled run error. No animations ever run under jsdom, so
// an empty list is the correct answer everywhere.
if (typeof Element !== "undefined" && !Element.prototype.getAnimations) {
    Element.prototype.getAnimations = () => [];
}

// With getAnimations defined, Base UI's useAnimationsFinished would switch
// to awaiting animation promises and defer popup/panel unmounts by a
// microtask. The flag is its supported switch to keep exit unmounts
// synchronous, matching what these tests assert against.
(
    globalThis as { BASE_UI_ANIMATIONS_DISABLED?: boolean }
).BASE_UI_ANIMATIONS_DISABLED = true;
