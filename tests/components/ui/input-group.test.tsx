// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { ReactNode } from "react";
import { createPortal } from "react-dom";
import { afterEach, describe, expect, it } from "vitest";

import {
    InputGroup,
    InputGroupAddon,
    InputGroupInput,
} from "@/components/ui/input-group";

function PortaledContent(): ReactNode {
    return createPortal(
        <div data-testid="portaled">hook output</div>,
        document.body
    );
}

function renderGroup(addon: ReactNode) {
    return render(
        <InputGroup>
            <InputGroupAddon>{addon}</InputGroupAddon>
            <InputGroupInput aria-label="commit summary" />
        </InputGroup>
    );
}

describe("InputGroupAddon", () => {
    afterEach(cleanup);

    it("ignores a press inside a portaled child, which is not in the DOM subtree", () => {
        const { getByTestId } = renderGroup(<PortaledContent />);
        const input = screen.getByLabelText("commit summary");

        const notPrevented = fireEvent.mouseDown(getByTestId("portaled"), {
            cancelable: true,
        });

        // A prevented press is what blocks the text selection a drag starts,
        // and the handler would move focus to the input on top of it.
        expect(notPrevented).toBe(true);
        expect(document.activeElement).not.toBe(input);
    });

    it("focuses the input for a press on the addon's own content", () => {
        const { getByText } = renderGroup(<span>prefix</span>);
        const input = screen.getByLabelText("commit summary");

        fireEvent.mouseDown(getByText("prefix"));

        expect(document.activeElement).toBe(input);
    });
});
