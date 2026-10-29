import { describe, expect, it } from "vitest";

import {
    isDefaultBranchName,
    pickBaseBranch,
} from "@/lib/repositories/default-branch";

describe("isDefaultBranchName", () => {
    it("matches main and master case-insensitively", () => {
        expect(isDefaultBranchName("main")).toBe(true);
        expect(isDefaultBranchName("Master")).toBe(true);
        expect(isDefaultBranchName("develop")).toBe(false);
    });
});

describe("pickBaseBranch", () => {
    it("prefers main, then master, then the first branch", () => {
        expect(
            pickBaseBranch(["develop", "master", "main"], "feature/thing")
        ).toBe("main");
        expect(pickBaseBranch(["develop", "master"], "feature/thing")).toBe(
            "master"
        );
        expect(pickBaseBranch(["zeta", "develop"], "feature/thing")).toBe(
            "zeta"
        );
    });

    it("never picks the head branch", () => {
        expect(pickBaseBranch(["main"], "main")).toBe("");
        expect(pickBaseBranch(["feature", "develop"], "feature")).toBe(
            "develop"
        );
    });
});
