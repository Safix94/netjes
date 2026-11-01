import { describe, expect, it } from "vitest";
import { buildAnimModel, thumbOrder } from "../lib/animModel";
import { launchOrder } from "../lib/sorter";
import { file } from "./fixtures";

describe("animModel", () => {
  it("animates every file under the cap, lays out the pile for that count and assigns slots in launch order", () => {
    const files = [...Array(30)].map((_, i) => file(i % 3 ? "fotos" : "pdfs"));
    const model = buildAnimModel(files, 160, 7);
    expect(model.anim).toHaveLength(30);
    expect(model.animated.size).toBe(30);
    expect(model.items).toHaveLength(30);
    for (const item of model.items) expect(item.radius).toBeGreaterThan(0);
    const byId = new Map(model.anim.map((a) => [a.id, a]));
    const seen = new Map<string, number>();
    for (const id of launchOrder(model.items)) {
      const a = byId.get(id)!;
      expect(a.slot).toBe(seen.get(a.category) ?? 0);
      seen.set(a.category, a.slot + 1);
    }
    expect(buildAnimModel(files, 160, 7)).toEqual(model);
  });

  it("caps at 600 tiles, keeps the rest silent and marks them with radius -1", () => {
    const files = [...Array(1000)].map((_, i) => file(i < 900 ? "fotos" : "pdfs"));
    const model = buildAnimModel(files, 160, 1);
    expect(model.anim).toHaveLength(600);
    expect(model.animated.size).toBe(600);
    expect(model.items.filter((i) => i.radius === -1)).toHaveLength(400);
    expect(model.anim.filter((a) => a.category === "pdfs").length).toBe(100);
    const radii = model.anim.map((a) => a.pile.radius);
    expect(Math.max(...radii)).toBeLessThanOrEqual(176);
    expect(Math.min(...radii)).toBeGreaterThan(50);
  });

  it("thumbOrder lists the outer ring first", () => {
    const files = [...Array(50)].map(() => file("fotos"));
    const model = buildAnimModel(files, 160, 3);
    const order = thumbOrder(model.anim);
    expect(order).toHaveLength(50);
    const byId = new Map(model.anim.map((a) => [a.id, a]));
    for (let i = 1; i < order.length; i++) {
      expect(byId.get(order[i - 1])!.pile.radius).toBeGreaterThanOrEqual(byId.get(order[i])!.pile.radius);
    }
  });

  it("returns an empty model for an empty folder", () => {
    const model = buildAnimModel([], 160, 1);
    expect(model.anim).toEqual([]);
    expect(model.items).toEqual([]);
    expect(model.animated.size).toBe(0);
  });
});
