import { describe, expect, it } from "vitest";
import { makeRng } from "../lib/rng";
import {
  easeInOutCubic,
  flightParams,
  flightPoint,
  flightState,
} from "../lib/flight";

describe("flight", () => {
  it("flightPoint hits start and end and rises in between", () => {
    const f = { x: 0, y: 100 };
    const t = { x: 200, y: 300 };
    expect(flightPoint(f, t, 80, 0)).toEqual(f);
    expect(flightPoint(f, t, 80, 1)).toEqual(t);
    // De bezier (controle y = 20) stijgt boven het startpunt, maar bij e = .5 is y
    // (100 + 2 * 20 + 300) / 4 = 110: de boog is al aan het dalen naar het lagere doel.
    expect(flightPoint(f, t, 80, 0.25).y).toBeLessThan(100);
    // Bij een vlakke vlucht ligt het midden van de boog duidelijk boven de rechte lijn.
    const level = flightPoint({ x: 0, y: 300 }, { x: 200, y: 300 }, 80, 0.5);
    expect(level.x).toBeCloseTo(100);
    expect(level.y).toBeLessThan(300);
  });

  it("easeInOutCubic is symmetric and monotonic", () => {
    expect(easeInOutCubic(0)).toBe(0);
    expect(easeInOutCubic(0.5)).toBeCloseTo(0.5);
    expect(easeInOutCubic(1)).toBe(1);
    expect(easeInOutCubic(0.25) + easeInOutCubic(0.75)).toBeCloseTo(1);
    expect(easeInOutCubic(0.25)).toBeLessThan(easeInOutCubic(0.6));
  });

  it("flightParams stays in the approved ranges", () => {
    const rnd = makeRng(3);
    for (let i = 0; i < 200; i++) {
      const p = flightParams(rnd);
      expect(p.dur).toBeGreaterThanOrEqual(760);
      expect(p.dur).toBeLessThan(1060);
      expect(p.lift).toBeGreaterThanOrEqual(40);
      expect(p.lift).toBeLessThan(120);
      expect(p.jitter).toBeGreaterThanOrEqual(0);
      expect(p.jitter).toBeLessThan(90);
    }
  });

  it("flightState starts at rest on the start and ends on the target", () => {
    const from = { x: 0, y: 100 };
    const to = { x: 200, y: 300 };
    const start = flightState(from, to, 0, 20, 0.5, 80);
    expect(start).toMatchObject({ x: 0, y: 100, rot: 20, scale: 1, speed: 0, vx: 0, vy: 0 });
    const end = flightState(from, to, 1, 20, 0.5, 80);
    expect(end.x).toBeCloseTo(200);
    expect(end.y).toBeCloseTo(300);
    expect(end.rot).toBeCloseTo(0);
    expect(end.scale).toBeCloseTo(0.5);
  });

  it("flightState reports speed and a unit direction while moving", () => {
    const from = { x: 0, y: 100 };
    const to = { x: 200, y: 300 };
    const mid = flightState(from, to, 0.5, 0, 1, 80);
    expect(mid.speed).toBeGreaterThan(0);
    expect(Math.hypot(mid.vx, mid.vy)).toBeCloseTo(1);
    expect(mid.vx).toBeGreaterThan(0);
  });
});
