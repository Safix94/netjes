import { describe, expect, it } from "vitest";
import { copy } from "../copy";

describe("copy", () => {
  it("has no dashes, exclamation marks or English leftovers", () => {
    const all = JSON.stringify(
      Object.values(copy).map((v) => (typeof v === "function" ? (v as (x: never) => string)(2 as never) : v)),
    );
    expect(all).not.toMatch(/[—–!]/);
    expect(all).not.toMatch(/\b(Organize|Undo|Folder|Done|Ready)\b/);
  });
});
