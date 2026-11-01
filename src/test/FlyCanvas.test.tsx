import "./canvas-mock";
import { act, render } from "@testing-library/react";
import { createRef } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { callsOf } from "./canvas-mock";
import { FlyCanvas, type AnimFile } from "../components/FlyCanvas";
import { layoutPile } from "../lib/pile";
import type { Outcome } from "../lib/sorter";

// Eén rAF-tick per aanroep van `tick()`, zodat de test de lus zelf bestuurt.
let frameCallbacks: FrameRequestCallback[] = [];
let clock = 0;

function tick(ms = 16) {
  clock += ms;
  const cbs = frameCallbacks;
  frameCallbacks = [];
  act(() => {
    cbs.forEach((cb) => cb(clock));
  });
}

function makeFiles(n: number, launchAt?: number): AnimFile[] {
  return layoutPile(n, 160, 3).map((pile, id) => ({
    id,
    category: id % 2 ? "pdfs" : "fotos",
    pile,
    flight: { dur: 800, lift: 60, jitter: 0 },
    launchAt,
    slot: id,
    state: "pile" as const,
  }));
}

describe("FlyCanvas", () => {
  beforeEach(() => {
    clock = 0;
    frameCallbacks = [];
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
      frameCallbacks.push(cb);
      return frameCallbacks.length;
    });
    vi.stubGlobal("cancelAnimationFrame", () => undefined);
    vi.spyOn(performance, "now").mockImplementation(() => clock);
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe() {}
        disconnect() {}
      },
    );
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  it("draws the pile from sprites on one canvas and calls onLand once per landed tile", () => {
    const stageRef = createRef<HTMLDivElement>();
    const onLand = vi.fn();
    const onReturn = vi.fn();
    const files = makeFiles(6, 0);
    const outcomes = new Map<number, Outcome>(files.map((f) => [f.id, { id: f.id, ok: f.id !== 5 }]));
    const slot = new DOMRect(400, 600, 24, 24);
    const { container } = render(
      <div>
        <div ref={stageRef} />
        <FlyCanvas
          files={files}
          phase="sorting"
          stageRef={stageRef}
          getSlotRect={() => slot}
          onLand={onLand}
          onReturn={onReturn}
          outcomes={outcomes}
        />
      </div>,
    );
    const canvases = container.querySelectorAll("canvas");
    expect(canvases).toHaveLength(1);
    const canvas = canvases[0];

    tick(100); // in de vlucht
    expect(callsOf(canvas)).toContain("drawImage");
    expect(callsOf(canvas)).not.toContain("clip");
    expect(onLand).not.toHaveBeenCalled();

    tick(800); // alles is geland of keert terug
    tick(16);
    expect(onLand).toHaveBeenCalledTimes(5);
    expect(onReturn).not.toHaveBeenCalled();

    tick(600); // de terugvlucht van 600 ms is voorbij
    tick(16);
    expect(onReturn).toHaveBeenCalledTimes(1);
    expect(onReturn).toHaveBeenCalledWith(5);
    expect(onLand).toHaveBeenCalledTimes(5);
  });

  it("keeps rendering the pile in rest without landing anything", () => {
    const stageRef = createRef<HTMLDivElement>();
    const onLand = vi.fn();
    render(
      <div>
        <div ref={stageRef} />
        <FlyCanvas
          files={makeFiles(4)}
          phase="ready"
          stageRef={stageRef}
          getSlotRect={() => null}
          onLand={onLand}
          onReturn={() => undefined}
          outcomes={new Map()}
        />
      </div>,
    );
    tick(16);
    tick(16);
    expect(onLand).not.toHaveBeenCalled();
    expect(frameCallbacks.length).toBe(1);
  });
});
