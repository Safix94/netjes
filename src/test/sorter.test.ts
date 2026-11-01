import { describe, expect, it } from "vitest";
import {
  batches,
  landingFor,
  launchOrder,
  pickAnimated,
  schedule,
  type SortItem,
} from "../lib/sorter";

describe("sorter", () => {
  it("pickAnimated caps at 600 and spreads across categories", () => {
    const items: SortItem[] = Array.from({ length: 1000 }, (_, id) => ({
      id,
      category: id < 900 ? "fotos" : "pdfs",
      radius: id,
    }));
    const set = pickAnimated(items);
    expect(set.size).toBe(600);
    const pdfs = items.filter((i) => i.category === "pdfs" && set.has(i.id));
    expect(pdfs.length).toBeGreaterThanOrEqual(100);
  });

  it("pickAnimated keeps everything when under the cap", () => {
    const items: SortItem[] = [
      { id: 4, category: "a", radius: 1 },
      { id: 9, category: "b", radius: 2 },
    ];
    expect([...pickAnimated(items)]).toEqual([4, 9]);
    expect(pickAnimated(items, 1).size).toBe(1);
  });

  it("launchOrder puts the outer ring first", () => {
    const items: SortItem[] = [
      { id: 0, category: "a", radius: 10 },
      { id: 1, category: "a", radius: 150 },
      { id: 2, category: "a", radius: 80 },
    ];
    expect(launchOrder(items)).toEqual([1, 2, 0]);
  });

  it("launchOrder breaks ties by id and leaves the input untouched", () => {
    const items: SortItem[] = [
      { id: 5, category: "a", radius: 50 },
      { id: 2, category: "a", radius: 50 },
      { id: 3, category: "a", radius: 90 },
    ];
    expect(launchOrder(items)).toEqual([3, 2, 5]);
    expect(items.map((i) => i.id)).toEqual([5, 2, 3]);
  });

  it("schedule staggers by 12 ms plus jitter", () => {
    const order = [8, 3, 5, 1, 9, 4, 7];
    const jitter = (id: number) => id % 7;
    const t0 = 250;
    const m = schedule(order, t0, 12, jitter);
    expect(m.size).toBe(order.length);
    expect(m.get(order[5])).toBe(t0 + 60 + jitter(order[5]));
    expect(m.get(order[0])).toBe(t0 + jitter(order[0]));
  });

  it("batches chunks the order and keeps the remainder in a last batch", () => {
    const order = Array.from({ length: 95 }, (_, i) => i);
    const b = batches(order, 40);
    expect(b.map((x) => x.length)).toEqual([40, 40, 15]);
    expect(b.flat()).toEqual(order);
    expect(batches([], 40)).toEqual([]);
    expect(() => batches(order, 0)).toThrow(RangeError);
  });

  it("landingFor", () => {
    expect(landingFor(undefined, 0.5)).toBe("fly");
    expect(landingFor(undefined, 1)).toBe("hold");
    expect(landingFor({ id: 1, ok: true }, 1)).toBe("land");
    expect(landingFor({ id: 1, ok: false }, 1)).toBe("return");
    expect(landingFor({ id: 1, ok: true }, 0.5)).toBe("fly");
  });
});
