import { describe, expect, it } from "vitest";
import { makeRng } from "../lib/rng";
import { layoutPile, tileSize } from "../lib/pile";

describe("makeRng", () => {
  it("is deterministic per seed and stays in [0, 1)", () => {
    const a = makeRng(42);
    const b = makeRng(42);
    const seqA = Array.from({ length: 50 }, () => a());
    const seqB = Array.from({ length: 50 }, () => b());
    expect(seqA).toEqual(seqB);
    expect(seqA.every((v) => v >= 0 && v < 1)).toBe(true);
    const c = makeRng(43);
    expect(Array.from({ length: 50 }, () => c())).not.toEqual(seqA);
  });

  it("matches the approved mockup (opruimer-v3.html) for seed 7", () => {
    const r = makeRng(7);
    expect([r(), r(), r()]).toEqual([0.011704753153026104, 0.06195825757458806, 0.97690763277933]);
  });
});

describe("pile", () => {
  it("layoutPile is deterministic and leaves a hole", () => {
    const a = layoutPile(431, 160, 7);
    const b = layoutPile(431, 160, 7);
    expect(a).toEqual(b);
    expect(a).toHaveLength(431);
    expect(Math.min(...a.map((p) => p.radius))).toBeGreaterThan(50);
    expect(Math.max(...a.map((p) => p.radius))).toBeLessThanOrEqual(160 * 1.1);
  });

  it("layoutPile keeps position, rotation, size and phase in range", () => {
    const pile = layoutPile(431, 160, 7);
    const size = tileSize(431);
    for (const p of pile) {
      expect(Math.hypot(p.x, p.y)).toBeCloseTo(p.radius);
      expect(p.rot).toBeGreaterThanOrEqual(-25);
      expect(p.rot).toBeLessThanOrEqual(25);
      expect(p.size).toBeGreaterThanOrEqual(size * 0.85);
      expect(p.size).toBeLessThanOrEqual(size * 1.15);
      expect(p.phase).toBeGreaterThanOrEqual(0);
      expect(p.phase).toBeLessThan(2 * Math.PI);
    }
    expect(layoutPile(431, 160, 8)).not.toEqual(pile);
    expect(layoutPile(0, 160, 7)).toEqual([]);
  });

  it("tileSize shrinks with count", () => {
    expect(tileSize(66)).toBeCloseTo(54.06);
    expect(tileSize(431)).toBeCloseTo(21.21);
    expect(tileSize(2000)).toBe(16);
    expect(tileSize(0)).toBe(60);
  });
});
