import { describe, expect, test } from "vitest";

import {
    insetBackdropClipPath,
    parsePx,
    roundedRectPath,
} from "@/lib/backdrop/inset-backdrop-clip";

const INSET = { left: 0, top: 48, right: 800, bottom: 600 };
const INSET_RADIUS = 10;

function makeBump() {
    return {
        left: 100,
        right: 200,
        top: 30,
        radius: 6,
        leftWing: { left: 50, top: 34, right: 100, bottom: 48 },
        rightWing: { left: 200, top: 34, right: 250, bottom: 48 },
    };
}

describe("parsePx", () => {
    test("parses pixel strings", () => {
        expect(parsePx("12.5px")).toBe(12.5);
        expect(parsePx("0px")).toBe(0);
    });

    test("returns 0 for garbage", () => {
        expect(parsePx("auto")).toBe(0);
        expect(parsePx("")).toBe(0);
    });
});

describe("roundedRectPath", () => {
    test("traces a clockwise rounded rectangle", () => {
        const path = roundedRectPath(INSET, INSET_RADIUS);
        expect(path).toBe(
            "M 10 48 H 790 A 10 10 0 0 1 800 58 V 590 A 10 10 0 0 1 790 600 H 10 A 10 10 0 0 1 0 590 V 58 A 10 10 0 0 1 10 48 Z"
        );
    });

    test("clamps radius to half the shortest side", () => {
        const path = roundedRectPath(
            { left: 0, top: 0, right: 20, bottom: 4 },
            10
        );
        expect(path).toContain("A 2 2 0 0 1 20 2");
    });
});

describe("insetBackdropClipPath", () => {
    test("falls back to the plain rounded rect without a bump", () => {
        expect(insetBackdropClipPath(INSET, INSET_RADIUS, null)).toBe(
            roundedRectPath(INSET, INSET_RADIUS)
        );
    });

    test("traces the tab bump and wing fillets into the top edge", () => {
        const path = insetBackdropClipPath(INSET, INSET_RADIUS, makeBump());
        expect(path).toBe(
            "M 10 48 L 50 48 A 50 50 0 0 0 100 34 L 100 48 L 100 36 A 6 6 0 0 1 106 30 L 194 30 A 6 6 0 0 1 200 36 L 200 34 A 50 50 0 0 0 250 48 L 790 48 A 10 10 0 0 1 800 58 L 800 590 A 10 10 0 0 1 790 600 L 10 600 A 10 10 0 0 1 0 590 V 58 A 10 10 0 0 1 10 48 Z"
        );
    });

    test("skips wings when they are missing or zero-sized", () => {
        const noWings = insetBackdropClipPath(INSET, INSET_RADIUS, {
            ...makeBump(),
            leftWing: null,
            rightWing: null,
        });
        expect(noWings).toContain("L 100 36");
        expect(noWings).not.toContain("A 50 50");

        const zeroWing = insetBackdropClipPath(INSET, INSET_RADIUS, {
            ...makeBump(),
            rightWing: { left: 200, top: 48, right: 200, bottom: 48 },
        });
        // The right wing arc must vanish while the left one stays.
        expect(zeroWing).not.toContain("250 48");
        expect(zeroWing).toContain("A 50 50 0 0 0 100 34");
    });
});
