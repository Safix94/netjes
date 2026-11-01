import { describe, expect, it, vi } from "vitest";
import { flightState } from "../lib/flight";
import { layoutPile } from "../lib/pile";
import {
  HOLD_P,
  MAX_ANIMATED,
  RETURN_MS,
  Scene,
  type SceneFile,
  type SlotTarget,
} from "../lib/scene";
import type { Outcome } from "../lib/sorter";

const CENTER = { x: 500, y: 300 };
const FLIGHT = { dur: 1000, lift: 60, jitter: 0 };

function makeFiles(n: number, category = "fotos"): SceneFile[] {
  const pile = layoutPile(n, 160, 11);
  return pile.map((p, id) => ({ id, category, pile: p, flight: FLIGHT, slot: id, state: "pile" as const }));
}

const slotAt =
  (x: number, y: number, s = 24) =>
  (_cat: string, _slot: number): SlotTarget | null => ({ x, y, s });

function scene() {
  const onLand = vi.fn();
  const onReturn = vi.fn();
  const s = new Scene({ onLand, onReturn });
  return { s, onLand, onReturn };
}

describe("scene", () => {
  it("caps the pile at 600 tiles and draws the outer ring first", () => {
    const { s } = scene();
    s.setFiles(makeFiles(1000));
    const { poses } = s.frame(0, CENTER, false, new Map(), slotAt(0, 0));
    expect(poses).toHaveLength(MAX_ANIMATED);
    const files = s.files;
    for (let i = 1; i < files.length; i++) {
      expect(files[i - 1].pile.radius).toBeGreaterThanOrEqual(files[i].pile.radius);
    }
  });

  it("bobs in rest and sits still while sorting before launch", () => {
    const { s } = scene();
    const files = makeFiles(3);
    s.setFiles(files);
    const rest = s.frame(1234, CENTER, false, new Map(), slotAt(0, 0));
    const f = files.find((x) => x.id === rest.poses[0].id)!;
    expect(rest.poses[0].mode).toBe("pile");
    expect(rest.poses[0].x).toBeCloseTo(CENTER.x + f.pile.x);
    expect(rest.poses[0].y).not.toBeCloseTo(CENTER.y + f.pile.y, 3);
    expect(Math.abs(rest.poses[0].y - (CENTER.y + f.pile.y))).toBeLessThan(2);

    for (const file of files) file.launchAt = 5000;
    const sorting = s.frame(1234, CENTER, true, new Map(), slotAt(0, 0));
    const g = files.find((x) => x.id === sorting.poses[0].id)!;
    expect(sorting.poses[0].y).toBeCloseTo(CENTER.y + g.pile.y);
    expect(sorting.poses[0].rot).toBeCloseTo(g.pile.rot);
  });

  it("flies along the arc and holds at .98 without an outcome", () => {
    const { s, onLand } = scene();
    const [f] = makeFiles(1);
    f.launchAt = 0;
    s.setFiles([f]);
    const target = slotAt(900, 700, 24);
    const from = { x: CENTER.x + f.pile.x, y: CENTER.y + f.pile.y };
    const mid = s.frame(500, CENTER, true, new Map(), target).poses[0];
    expect(mid.mode).toBe("fly");
    const want = flightState(from, { x: 900, y: 700 }, 0.5, f.pile.rot, 24 / f.pile.size, FLIGHT.lift);
    expect(mid.x).toBeCloseTo(want.x);
    expect(mid.y).toBeCloseTo(want.y);
    expect(mid.speed).toBeGreaterThan(0);

    const hold = s.frame(1500, CENTER, true, new Map(), target).poses[0];
    expect(hold.mode).toBe("hold");
    const wantHold = flightState(from, { x: 900, y: 700 }, HOLD_P, f.pile.rot, 24 / f.pile.size, FLIGHT.lift);
    expect(hold.x).toBeCloseTo(wantHold.x);
    expect(hold.y).toBeCloseTo(wantHold.y);
    expect(hold.speed).toBe(0);
    expect(onLand).not.toHaveBeenCalled();
  });

  it("lands once, fills the slot and fires onLand", () => {
    const { s, onLand } = scene();
    const files = makeFiles(2);
    files[0].launchAt = 0;
    files[1].launchAt = 0;
    files[1].slot = 12;
    s.setFiles(files);
    const outcomes = new Map<number, Outcome>([
      [0, { id: 0, ok: true }],
      [1, { id: 1, ok: true }],
    ]);
    const a = s.frame(1000, CENTER, true, outcomes, slotAt(900, 700, 24));
    const b = s.frame(1016, CENTER, true, outcomes, slotAt(900, 700, 24));
    expect(onLand).toHaveBeenCalledTimes(2);
    expect(onLand).toHaveBeenCalledWith(0);
    expect(onLand).toHaveBeenCalledWith(1);
    expect(a.poses).toHaveLength(0);
    expect(b.poses).toHaveLength(0);
    // Enkel vakje 0 tot 11 wordt gevuld; vakje 12 verdwijnt in de kaart.
    expect(b.landed).toEqual([{ id: 0, x: 900, y: 700, s: 24 }]);
  });

  it("returns to the pile in 600 ms and fires onReturn once", () => {
    const { s, onReturn, onLand } = scene();
    const [f] = makeFiles(1);
    f.launchAt = 0;
    s.setFiles([f]);
    const outcomes = new Map<number, Outcome>([[0, { id: 0, ok: false, error: "in gebruik" }]]);
    const target = slotAt(900, 700, 24);
    const start = s.frame(1000, CENTER, true, outcomes, target).poses[0];
    expect(start.mode).toBe("return");
    expect(start.x).toBeCloseTo(900);
    expect(start.y).toBeCloseTo(700);
    const mid = s.frame(1000 + RETURN_MS / 2, CENTER, true, outcomes, target).poses[0];
    expect(mid.mode).toBe("return");
    expect(mid.x).toBeLessThan(900);
    expect(onReturn).not.toHaveBeenCalled();
    const back = s.frame(1000 + RETURN_MS, CENTER, true, outcomes, target).poses[0];
    expect(back.mode).toBe("pile");
    expect(back.x).toBeCloseTo(CENTER.x + f.pile.x);
    expect(back.y).toBeCloseTo(CENTER.y + f.pile.y);
    s.frame(2000, CENTER, true, outcomes, target);
    expect(onReturn).toHaveBeenCalledTimes(1);
    expect(onReturn).toHaveBeenCalledWith(0);
    expect(onLand).not.toHaveBeenCalled();
  });

  it("starts the finale once after the last land or return and resets cleanly", () => {
    const { s } = scene();
    const files = makeFiles(2);
    files[0].launchAt = 0;
    files[1].launchAt = 0;
    s.setFiles(files);
    const outcomes = new Map<number, Outcome>([[0, { id: 0, ok: true }]]);
    const target = slotAt(900, 700, 24);
    expect(s.frame(1000, CENTER, true, outcomes, target).finaleAt).toBeUndefined();
    outcomes.set(1, { id: 1, ok: false });
    expect(s.frame(1100, CENTER, true, outcomes, target).finaleAt).toBeUndefined();
    const done = s.frame(1100 + RETURN_MS, CENTER, true, outcomes, target);
    expect(done.finaleAt).toBe(1100 + RETURN_MS);
    expect(s.frame(3000, CENTER, true, outcomes, target).finaleAt).toBe(1100 + RETURN_MS);
    s.reset();
    expect(s.frame(3000, CENTER, false, new Map(), target).finaleAt).toBeUndefined();
    expect(s.frame(3000, CENTER, false, new Map(), target).landed).toEqual([]);
  });

  it("still lands when no slot rect exists, below the stage", () => {
    const { s, onLand } = scene();
    const [f] = makeFiles(1);
    f.launchAt = 0;
    s.setFiles([f]);
    const outcomes = new Map<number, Outcome>([[0, { id: 0, ok: true }]]);
    const mid = s.frame(500, CENTER, true, outcomes, () => null).poses[0];
    expect(mid.mode).toBe("fly");
    const r = s.frame(1000, CENTER, true, outcomes, () => null);
    expect(onLand).toHaveBeenCalledWith(0);
    expect(r.landed).toEqual([]);
  });

  it("does not start a finale when nothing was launched", () => {
    const { s } = scene();
    s.setFiles(makeFiles(3));
    expect(s.frame(100, CENTER, true, new Map(), slotAt(0, 0)).finaleAt).toBeUndefined();
  });
});
