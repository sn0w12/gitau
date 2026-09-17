import { describe, expect, it, beforeEach } from "vitest";

import {
    ensureRepo,
    moveRepo,
    registerOpen,
    removeRepo,
    repositoryStore,
    selectRepoEntries,
} from "@/stores/repository-store";

function reset() {
    repositoryStore.setState(() => ({ entries: new Map() }));
}

function paths() {
    return selectRepoEntries(repositoryStore.state).map((entry) => entry.path);
}

describe("repository store ordering", () => {
    beforeEach(reset);

    it("keeps repos in insertion order and appends new ones", () => {
        ensureRepo("/a");
        ensureRepo("/b");
        expect(paths()).toEqual(["/a", "/b"]);

        ensureRepo("/c");
        expect(paths()).toEqual(["/a", "/b", "/c"]);
    });

    it("dedupes case-insensitively and keeps the first spelling", () => {
        ensureRepo("/Repo");
        ensureRepo("/repo");
        expect(paths()).toEqual(["/Repo"]);
    });

    it("moves a repo to a new position", () => {
        ensureRepo("/a");
        ensureRepo("/b");
        ensureRepo("/c");

        moveRepo("/c", 0);
        expect(paths()).toEqual(["/c", "/a", "/b"]);

        moveRepo("/c", 2);
        expect(paths()).toEqual(["/a", "/b", "/c"]);
    });

    it("matches paths case-insensitively", () => {
        ensureRepo("/Alpha");
        ensureRepo("/beta");

        moveRepo("/alpha", 1);
        expect(paths()).toEqual(["/beta", "/Alpha"]);
    });

    it("clamps out-of-range indexes", () => {
        ensureRepo("/a");
        ensureRepo("/b");

        moveRepo("/a", 99);
        expect(paths()).toEqual(["/b", "/a"]);

        moveRepo("/a", -5);
        expect(paths()).toEqual(["/a", "/b"]);
    });

    it("no-ops for unknown paths, same index, and single-item lists", () => {
        ensureRepo("/a");
        const before = repositoryStore.state;

        moveRepo("/missing", 0);
        moveRepo("/a", 0);
        expect(repositoryStore.state).toBe(before);
    });
});

describe("repository store removal", () => {
    beforeEach(reset);

    it("removes an entry and keeps the rest in order", () => {
        ensureRepo("/a");
        ensureRepo("/b");
        ensureRepo("/c");

        removeRepo("/b");
        expect(paths()).toEqual(["/a", "/c"]);
    });

    it("matches paths case-insensitively and drops the canonical entry", () => {
        ensureRepo("/Repo");
        ensureRepo("/other");

        removeRepo("/repo");
        expect(paths()).toEqual(["/other"]);
    });

    it("no-ops for unknown paths", () => {
        ensureRepo("/a");
        const before = repositoryStore.state;

        removeRepo("/missing");
        expect(repositoryStore.state).toBe(before);
    });

    it("drops the backend binding together with the entry", () => {
        registerOpen("/a", 7);

        removeRepo("/a");
        expect(repositoryStore.state.entries.size).toBe(0);
    });
});
