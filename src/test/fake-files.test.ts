import { describe, expect, it } from "vitest";
import { FAKE_CATEGORIES, makeFakeFiles } from "../dev/fake-files";

describe("fake-files", () => {
  it("builds n files with a downloads-like spread and a few duplicates", () => {
    const files = makeFakeFiles(431, 7);
    expect(files).toHaveLength(431);
    expect(new Set(files.map((f) => f.id)).size).toBe(431);
    const count = (cat: string) => files.filter((f) => f.category === cat).length;
    expect(count("fotos")).toBeGreaterThan(431 * 0.3);
    expect(count("screenshots")).toBeGreaterThan(10);
    expect(count("pdfs")).toBeGreaterThan(10);
    expect(count("duplicaten")).toBeGreaterThanOrEqual(3);
    expect(count("duplicaten")).toBeLessThan(40);
    for (const f of files) expect(FAKE_CATEGORIES.some((c) => c.id === f.category)).toBe(true);
    expect(makeFakeFiles(431, 7)).toEqual(files);
  });

  it("scales to 1000 files and keeps every category represented", () => {
    const files = makeFakeFiles(1000, 1);
    expect(files).toHaveLength(1000);
    const cats = new Set(files.map((f) => f.category));
    expect(cats.size).toBe(FAKE_CATEGORIES.length);
    expect(files.filter((f) => f.name.endsWith(".png") || f.name.endsWith(".jpg")).length).toBeGreaterThan(400);
  });
});
