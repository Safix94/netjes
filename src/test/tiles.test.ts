import "./canvas-mock";
import { describe, expect, it } from "vitest";
import { callsOf } from "./canvas-mock";
import { CATEGORIES, SpriteCache, drawTypeTile, readPalette, spriteKey } from "../lib/tiles";

describe("tiles", () => {
  it("spriteKey changes when a thumbnail arrives", () => {
    const before = spriteKey({ id: 12, category: "fotos" });
    const after = spriteKey({ id: 12, category: "fotos", thumb: "C:\cache\abc.png" });
    expect(after).not.toBe(before);
    // Twee foto's zonder miniatuur delen een sprite; met verschillende miniaturen niet.
    expect(spriteKey({ id: 3, category: "fotos" })).toBe(spriteKey({ id: 6, category: "fotos" }));
    expect(spriteKey({ id: 3, category: "fotos", thumb: "a.png" })).not.toBe(
      spriteKey({ id: 3, category: "fotos", thumb: "b.png" }),
    );
    expect(spriteKey({ id: 3, category: "pdfs" })).not.toBe(spriteKey({ id: 3, category: "documenten" }));
  });

  it("drawTypeTile draws without throwing for every category", () => {
    expect(CATEGORIES.length).toBe(16);
    for (const category of [...CATEGORIES, "onbekend"]) {
      const canvas = document.createElement("canvas");
      canvas.width = canvas.height = 64;
      const ctx = canvas.getContext("2d")!;
      expect(() => drawTypeTile(ctx, category, 7, 64)).not.toThrow();
      expect(callsOf(canvas)).toContain("fillRect");
    }
  });

  it("SpriteCache bakes a sprite once per key and clears", () => {
    const cache = new SpriteCache(80);
    let draws = 0;
    const draw = (c: HTMLCanvasElement) => {
      draws++;
      c.getContext("2d")!.fillRect(0, 0, c.width, c.height);
    };
    const a = cache.get("fotos|0", draw);
    const b = cache.get("fotos|0", draw);
    expect(a).toBe(b);
    expect(a.width).toBe(80);
    expect(a.height).toBe(80);
    expect(draws).toBe(1);
    // Schaduw en rand zijn in de sprite gebakken, niet per frame.
    expect(callsOf(a)).toContain("drawImage");
    cache.clear();
    cache.get("fotos|0", draw);
    expect(draws).toBe(2);
  });

  it("readPalette falls back to the brand colours when no css variables are set", () => {
    const p = readPalette(document.documentElement);
    expect(p.ink).toBe("#12150F");
    expect(p.lime).toBe("#CDF464");
    expect(p.paper).toBe("#F4F6EE");
  });
});
