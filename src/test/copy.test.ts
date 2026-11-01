import { describe, expect, it } from "vitest";
import { copy } from "../copy";

describe("copy", () => {
  it("has no dashes, exclamation marks or English leftovers", () => {
    const all = JSON.stringify(Object.values(copy).map((v) => (typeof v === "function" ? v(2) : v)));
    expect(all).not.toMatch(/[—–!]/);
    expect(all).not.toMatch(/\b(Organize|Undo|Folder|Done|Ready)\b/);
  });
});
