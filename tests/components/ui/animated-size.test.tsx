// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { createElement } from "react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";

const captured = vi.hoisted(() => ({
    props: [] as Array<Record<string, unknown>>,
}));

vi.mock("motion/react", () => ({
    motion: {
        create: (tag: string) => {
            const Component = ({
                children,
                ...props
            }: { children?: ReactNode } & Record<string, unknown>) => {
                captured.props.push(props);
                return createElement(tag, null, children);
            };
            return Component;
        },
    },
    useReducedMotion: () => false,
}));

import { AnimatedSize } from "@/components/ui/animated-size";

afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
    captured.props.length = 0;
});

describe("AnimatedSize", () => {
    it("animates both axes by default", () => {
        render(<AnimatedSize>content</AnimatedSize>);

        expect(captured.props.at(-1)?.animate).toEqual({
            width: "auto",
            height: "auto",
        });
    });

    it("opts out of width", () => {
        render(<AnimatedSize width={false}>content</AnimatedSize>);

        expect(captured.props.at(-1)?.animate).toEqual({ height: "auto" });
    });

    it("opts out of height", () => {
        render(<AnimatedSize height={false}>content</AnimatedSize>);

        expect(captured.props.at(-1)?.animate).toEqual({ width: "auto" });
    });

    it("uses a custom transition", () => {
        const transition = { duration: 0.4, ease: [0.1, 0.2, 0.3, 1] } as const;
        render(<AnimatedSize transition={transition}>content</AnimatedSize>);

        expect(captured.props.at(-1)?.transition).toBe(transition);
    });

    it("measures the inner box", () => {
        vi.spyOn(
            HTMLElement.prototype,
            "getBoundingClientRect"
        ).mockReturnValue({
            width: 120,
            height: 40,
        } as DOMRect);

        render(<AnimatedSize>content</AnimatedSize>);

        expect(captured.props.at(-1)?.animate).toEqual({
            width: 120,
            height: 40,
        });
    });

    it("renders the requested element", () => {
        const { container } = render(
            <AnimatedSize as="section" innerAs="span">
                content
            </AnimatedSize>
        );

        expect(container.firstElementChild?.tagName).toBe("SECTION");
        expect(container.querySelector("section > span")).not.toBeNull();
    });
});
