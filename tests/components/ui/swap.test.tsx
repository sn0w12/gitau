// @vitest-environment jsdom
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { Swap } from "@/components/ui/swap";

afterEach(cleanup);

describe("Swap mount", () => {
    it("starts both sides in their target state", () => {
        const { container } = render(
            <Swap when={false} on={<span>on</span>} off={<span>off</span>} />
        );

        const visible = container.querySelector('[aria-hidden="false"]')!;
        const hidden = container.querySelector('[aria-hidden="true"]')!;
        expect(visible.getAttribute("style")).toContain("opacity: 1");
        expect(hidden.getAttribute("style")).toContain("opacity: 0");
    });
});
