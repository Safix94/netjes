export interface SortItem {
  id: number;
  category: string;
  radius: number;
}

/**
 * Kiest welke bestanden echt vliegen (max 600). Boven het maximum nemen we
 * round-robin één item per categorie, in invoervolgorde, zodat elke categorie
 * een eerlijk deel houdt.
 */
export function pickAnimated(items: SortItem[], max = 600): Set<number> {
  if (items.length <= max) return new Set(items.map((item) => item.id));
  const groups = new Map<string, number[]>();
  for (const item of items) {
    const group = groups.get(item.category);
    if (group) group.push(item.id);
    else groups.set(item.category, [item.id]);
  }
  const queues = [...groups.values()];
  const picked = new Set<number>();
  for (let round = 0; picked.size < max; round++) {
    let took = false;
    for (const queue of queues) {
      if (round < queue.length && picked.size < max) {
        picked.add(queue[round]);
        took = true;
      }
    }
    if (!took) break;
  }
  return picked;
}

/** Ids met de grootste radius eerst (buitenste ring), bij gelijke radius oplopend op id. */
export function launchOrder(items: SortItem[]): number[] {
  return [...items].sort((a, b) => b.radius - a.radius || a.id - b.id).map((item) => item.id);
}

/** Lanceertijdstip per id: t0 + index * stagger + jitter(id). */
export function schedule(
  order: number[],
  t0: number,
  stagger: number,
  jitter: (id: number) => number,
): Map<number, number> {
  const launchAt = new Map<number, number>();
  order.forEach((id, index) => {
    launchAt.set(id, t0 + index * stagger + jitter(id));
  });
  return launchAt;
}

/** Knipt de lanceervolgorde in batches; de laatste mag kleiner zijn. */
export function batches(order: number[], size: number): number[][] {
  if (!(size >= 1)) throw new RangeError("batchgrootte moet minstens 1 zijn");
  const out: number[][] = [];
  for (let i = 0; i < order.length; i += size) out.push(order.slice(i, i + size));
  return out;
}

export type Outcome = { id: number; ok: boolean; dest?: string; error?: string };

/** p < 1: vliegen. Daarna wachten zolang de uitkomst ontbreekt, anders landen (ok) of terugkeren (mislukt). */
export function landingFor(outcome: Outcome | undefined, p: number): "fly" | "hold" | "land" | "return" {
  if (p < 1) return "fly";
  if (!outcome) return "hold";
  return outcome.ok ? "land" : "return";
}
