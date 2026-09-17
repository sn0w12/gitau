import { describe, expect, it } from "vitest";

import {
    TAB_SWAP_DEAD_ZONE_PX,
    createTabCollisionDetector,
} from "@/components/titlebar/tab-collision-detector";

const detector = createTabCollisionDetector();

interface TabFixture {
    id: string;
    index: number;
    centerX: number;
}

function makeDroppable(tab: TabFixture) {
    return {
        id: tab.id,
        index: tab.index,
        sortable: { id: tab.id, index: tab.index },
        shape: { center: { x: tab.centerX, y: 14 } },
    };
}

function makeInput(options: {
    source: TabFixture;
    draggedCenterX: number | null;
    droppable: ReturnType<typeof makeDroppable>;
}) {
    const { source, draggedCenterX, droppable } = options;
    const input = {
        droppable,
        dragOperation: {
            source: {
                id: source.id,
                index: source.index,
                sortable: { id: source.id, index: source.index },
            },
            shape:
                draggedCenterX === null
                    ? null
                    : {
                          current: { center: { x: draggedCenterX, y: 14 } },
                      },
        },
    };
    return input as unknown as Parameters<typeof detector>[0];
}

// Adjacent 80px tabs with a 4px gap: centers 100 apart.
const A: TabFixture = { id: "a", index: 0, centerX: 100 };
const B: TabFixture = { id: "b", index: 1, centerX: 200 };

describe("tab collision detector", () => {
    it("ignores the dragged tab itself", () => {
        const collision = detector(
            makeInput({
                source: A,
                draggedCenterX: 500,
                droppable: makeDroppable(A),
            })
        );
        expect(collision).toBeNull();
    });

    it("does not fire before crossing a neighbor's center by the dead zone", () => {
        // Dragging A right toward B: 1px of rectangle overlap happens far
        // before A's center passes B's center + dead zone.
        for (const x of [145, 180, 207, B.centerX + TAB_SWAP_DEAD_ZONE_PX]) {
            const collision = detector(
                makeInput({
                    source: A,
                    draggedCenterX: x,
                    droppable: makeDroppable(B),
                })
            );
            expect(collision, `draggedCenterX=${x}`).toBeNull();
        }
    });

    it("fires once the right neighbor's center is genuinely crossed", () => {
        const collision = detector(
            makeInput({
                source: A,
                draggedCenterX: B.centerX + TAB_SWAP_DEAD_ZONE_PX + 1,
                droppable: makeDroppable(B),
            })
        );
        expect(collision?.id).toBe("b");
        expect(collision?.value).toBeGreaterThan(0);
    });

    it("mirrors the rule for the left neighbor", () => {
        const insideDeadZone = detector(
            makeInput({
                source: B,
                draggedCenterX: A.centerX - TAB_SWAP_DEAD_ZONE_PX,
                droppable: makeDroppable(A),
            })
        );
        expect(insideDeadZone).toBeNull();

        const crossed = detector(
            makeInput({
                source: B,
                draggedCenterX: A.centerX - TAB_SWAP_DEAD_ZONE_PX - 1,
                droppable: makeDroppable(A),
            })
        );
        expect(crossed?.id).toBe("a");
    });

    it("does not swap a wide tab onto a narrow one at first touch", () => {
        // Regression: the default detector swaps as soon as rectangles touch,
        // so a 240px-wide tab teleported over an 80px tab after ~1px of drag.
        // Center-based detection ignores mere edge contact.
        const wide: TabFixture = { id: "wide", index: 0, centerX: 20 };
        const narrow = makeDroppable(B);
        const collision = detector(
            makeInput({
                source: wide,
                draggedCenterX: wide.centerX + 100,
                droppable: narrow,
            })
        );
        expect(collision).toBeNull();
    });

    it("stays stable after a reorder until the moved tab's new center is re-crossed", () => {
        // Post-swap geometry: B took C's slot (index 2), C slid left with
        // center 206. The pointer has not moved from 300.
        const bAfterSwap: TabFixture = { id: "b", index: 2, centerX: 300 };
        const cAfterSwap: TabFixture = { id: "c", index: 1, centerX: 206 };

        const stillOverC = detector(
            makeInput({
                source: bAfterSwap,
                draggedCenterX: 300,
                droppable: makeDroppable(cAfterSwap),
            })
        );
        expect(stillOverC).toBeNull();

        // Travelling back left across C's new center minus the dead zone
        // legitimately restores the previous order.
        const hysteresisBand = detector(
            makeInput({
                source: bAfterSwap,
                draggedCenterX: cAfterSwap.centerX - TAB_SWAP_DEAD_ZONE_PX + 1,
                droppable: makeDroppable(cAfterSwap),
            })
        );
        expect(hysteresisBand).toBeNull();

        const recrossed = detector(
            makeInput({
                source: bAfterSwap,
                draggedCenterX: cAfterSwap.centerX - TAB_SWAP_DEAD_ZONE_PX - 1,
                droppable: makeDroppable(cAfterSwap),
            })
        );
        expect(recrossed?.id).toBe("c");
    });

    it("prefers the nearest crossed candidate", () => {
        // Dragged center sits at 450: both candidates are crossed, but the
        // tab centered at 400 is the nearer one and must score higher.
        const farther: TabFixture = { id: "farther", index: 1, centerX: 200 };
        const nearer: TabFixture = { id: "nearer", index: 2, centerX: 400 };

        const fartherCollision = detector(
            makeInput({
                source: A,
                draggedCenterX: 450,
                droppable: makeDroppable(farther),
            })
        )!;
        const nearerCollision = detector(
            makeInput({
                source: A,
                draggedCenterX: 450,
                droppable: makeDroppable(nearer),
            })
        )!;

        expect(nearerCollision.value).toBeGreaterThan(fartherCollision.value);
    });

    it("returns null when shapes or indices are unavailable", () => {
        const noShape = detector(
            makeInput({
                source: A,
                draggedCenterX: null,
                droppable: makeDroppable(B),
            })
        );
        expect(noShape).toBeNull();

        const unindexed = {
            id: "b",
            sortable: { id: "b" },
            shape: { center: { x: 200, y: 14 } },
        };
        const noIndex = detector(
            makeInput({
                source: A,
                draggedCenterX: 500,
                droppable: unindexed as never,
            })
        );
        expect(noIndex).toBeNull();
    });
});
