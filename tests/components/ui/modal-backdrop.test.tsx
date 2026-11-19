// @vitest-environment jsdom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { describe, expect, it } from "vitest";

import { Dialog, DialogPopup, DialogTitle } from "@/components/ui/dialog";
import { ModalBackdrop } from "@/components/ui/modal-backdrop";

function flush(): Promise<void> {
    return act(async () => {
        await new Promise((resolve) => setTimeout(resolve, 20));
    });
}

describe("ModalBackdrop", () => {
    it("stays active while one dialog replaces another", async () => {
        const container = document.createElement("div");
        document.body.appendChild(container);
        const root = createRoot(container);

        const render = (first: boolean, second: boolean) => {
            act(() => {
                root.render(
                    <>
                        <ModalBackdrop />
                        <Dialog open={first}>
                            <DialogPopup>
                                <DialogTitle>First</DialogTitle>
                            </DialogPopup>
                        </Dialog>
                        <Dialog open={second}>
                            <DialogPopup>
                                <DialogTitle>Second</DialogTitle>
                            </DialogPopup>
                        </Dialog>
                    </>
                );
            });
        };

        render(true, false);
        await flush();
        const backdrop = document.querySelector<HTMLElement>(
            '[data-slot="modal-backdrop"]'
        );
        const isActive = () =>
            backdrop?.classList.contains("backdrop-blur-sm") ?? false;
        expect(isActive()).toBe(true);

        render(false, true);
        await flush();
        expect(isActive()).toBe(true);

        render(false, false);
        await flush();
        expect(isActive()).toBe(false);

        act(() => root.unmount());
        container.remove();
    });
});
