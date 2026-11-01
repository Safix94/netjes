import { describe, expect, it, vi } from "vitest";
import type { MoveOutcome, OrganizePlan } from "../lib/api";
import { launchOrder, runOrganize, type Outcome, type SortItem } from "../lib/sorter";

function items(n: number): SortItem[] {
  return Array.from({ length: n }, (_, id) => ({ id, category: id % 3 ? "fotos" : "pdfs", radius: (id * 37) % 200 }));
}

function fakeApi(ids: number[], failing: number[] = []) {
  const beginOrganize = vi.fn(
    async (_root: string): Promise<OrganizePlan> => ({
      moves: ids.map((id) => ({ id, src: `C:\\d\\${id}`, dest: `C:\\d\\m\\${id}`, category: "fotos" as const })),
      folders_created: ["Foto's"],
    }),
  );
  const moveFiles = vi.fn(async (_root: string, batch: number[]): Promise<MoveOutcome[]> =>
    batch.map((id) =>
      failing.includes(id)
        ? { id, ok: false, dest: null, error: "in gebruik" }
        : { id, ok: true, dest: `C:\\d\\m\\${id}`, error: null },
    ),
  );
  return { beginOrganize, moveFiles };
}

describe("runOrganize", () => {
  it("requests batches in launch order and reports every outcome", async () => {
    const files = items(100);
    const api = fakeApi(files.map((f) => f.id), [7]);
    const outcomes: Outcome[] = [];
    let scheduled: Map<number, number> | null = null;
    let animated: Set<number> | null = null;
    const result = await runOrganize({
      root: "C:\\d",
      files,
      api,
      now: () => 0,
      stagger: 0,
      sleep: async () => undefined,
      onSchedule: (launchAt, set) => {
        scheduled = launchAt;
        animated = set;
      },
      onOutcome: (o) => outcomes.push(o),
    });
    expect(api.beginOrganize).toHaveBeenCalledWith("C:\\d");
    const order = launchOrder(files);
    const batchesSent = api.moveFiles.mock.calls.map((c) => c[1]);
    expect(batchesSent.map((b) => b.length)).toEqual([40, 40, 20]);
    expect(batchesSent.flat()).toEqual(order);
    expect(outcomes).toHaveLength(100);
    expect(outcomes.find((o) => o.id === 7)).toMatchObject({ id: 7, ok: false, error: "in gebruik" });
    expect(outcomes.find((o) => o.id === 8)).toMatchObject({ id: 8, ok: true, dest: "C:\\d\\m\\8" });
    expect(result.failed.map((f) => f.id)).toEqual([7]);
    expect(result.moved).toBe(99);
    expect(scheduled!.size).toBe(100);
    expect(animated!.size).toBe(100);
    // Het schema begint 250 ms na nu en volgt de lanceervolgorde.
    expect(scheduled!.get(order[0])).toBeGreaterThanOrEqual(250);
  });

  it("keeps un-animated files in the count", async () => {
    const files = items(700);
    const api = fakeApi(files.map((f) => f.id));
    let animated: Set<number> | null = null;
    const result = await runOrganize({
      root: "C:\\d",
      files,
      api,
      now: () => 0,
      stagger: 0,
      sleep: async () => undefined,
      onSchedule: (_launchAt, set) => {
        animated = set;
      },
      onOutcome: () => undefined,
    });
    expect(animated!.size).toBe(600);
    expect(result.moved).toBe(700);
    expect(api.moveFiles.mock.calls.flatMap((c) => c[1])).toHaveLength(700);
  });

  it("waits until 400 ms before the first tile of a batch leaves", async () => {
    const files = items(80);
    const api = fakeApi(files.map((f) => f.id));
    const waits: number[] = [];
    let launchAt: Map<number, number> | null = null;
    await runOrganize({
      root: "C:\\d",
      files,
      api,
      now: () => 1000,
      stagger: 12,
      jitter: () => 0,
      sleep: async (ms) => {
        waits.push(ms);
      },
      onSchedule: (m) => {
        launchAt = m;
      },
      onOutcome: () => undefined,
    });
    const order = launchOrder(files);
    // Batch 0 vertrekt op 1250: aanvraag op 850, dus geen wachttijd. Batch 1 vertrekt op 1250 + 480.
    expect(launchAt!.get(order[40])).toBe(1250 + 40 * 12);
    expect(waits).toEqual([1250 + 480 - 400 - 1000]);
  });

  it("turns a refused batch into failed outcomes instead of throwing", async () => {
    const files = items(3);
    const api = fakeApi([0, 1, 2]);
    api.moveFiles.mockRejectedValueOnce("map niet bereikbaar");
    const outcomes: Outcome[] = [];
    const result = await runOrganize({
      root: "C:\\d",
      files,
      api,
      now: () => 0,
      stagger: 0,
      sleep: async () => undefined,
      onSchedule: () => undefined,
      onOutcome: (o) => outcomes.push(o),
    });
    expect(outcomes).toHaveLength(3);
    expect(outcomes.every((o) => !o.ok && o.error === "map niet bereikbaar")).toBe(true);
    expect(result.moved).toBe(0);
    expect(result.failed).toHaveLength(3);
  });

  it("only moves files that are in the plan", async () => {
    const files = items(5);
    const api = fakeApi([0, 1, 2]);
    const result = await runOrganize({
      root: "C:\\d",
      files,
      api,
      now: () => 0,
      stagger: 0,
      sleep: async () => undefined,
      onSchedule: () => undefined,
      onOutcome: () => undefined,
    });
    expect(api.moveFiles.mock.calls.flatMap((c) => c[1]).sort()).toEqual([0, 1, 2]);
    expect(result.moved).toBe(3);
  });
});
