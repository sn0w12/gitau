// @vitest-environment jsdom
import { act, cleanup, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { RelativeTime } from "@/components/relative-time";
import { formatRelativeDate } from "@/lib/utils";

const MINUTE_MS = 60_000;
const DAY_MS = 24 * 60 * MINUTE_MS;

describe("RelativeTime", () => {
    beforeEach(() => {
        vi.useFakeTimers();
        vi.setSystemTime(new Date("2026-05-01T12:00:00Z"));
    });

    afterEach(() => {
        cleanup();
        vi.useRealTimers();
    });

    it("renders the relative label", () => {
        const { container } = render(
            <RelativeTime value={Date.now() - 5 * MINUTE_MS} />
        );

        expect(container.textContent).toBe("5 minutes ago");
    });

    it("advances the label as the clock moves", () => {
        const { container } = render(
            <RelativeTime value={Date.now() - 5 * MINUTE_MS} />
        );

        act(() => vi.advanceTimersByTime(MINUTE_MS));

        expect(container.textContent).toBe("6 minutes ago");
    });

    it("reads the clock again when a fresh instance mounts", () => {
        const mountedAt = Date.now();
        const value = mountedAt - 5 * MINUTE_MS;
        const first = render(<RelativeTime value={value} />);
        expect(first.container.textContent).toBe("5 minutes ago");

        first.unmount();
        vi.setSystemTime(mountedAt + 2 * 60 * MINUTE_MS);

        const second = render(<RelativeTime value={value} />);
        expect(second.container.textContent).toBe("2 hours ago");
    });

    it("renders an old timestamp as a date and does not start the clock", () => {
        const value = Date.now() - 8 * DAY_MS;
        const { container } = render(<RelativeTime value={value} />);

        expect(container.textContent).toBe(formatRelativeDate(value));
        expect(vi.getTimerCount()).toBe(0);
    });

    it("stops the clock once the timestamp ages into a date", () => {
        const value = Date.now() - (7 * DAY_MS - MINUTE_MS);
        const { container } = render(<RelativeTime value={value} />);
        expect(container.textContent).toBe("6 days ago");
        expect(vi.getTimerCount()).toBe(1);

        act(() => vi.advanceTimersByTime(2 * MINUTE_MS));

        expect(container.textContent).toBe(formatRelativeDate(value));
        expect(vi.getTimerCount()).toBe(0);
    });
});
