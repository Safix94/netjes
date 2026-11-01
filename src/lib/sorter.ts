import { copy } from "../copy";
import { LOG_SAVE_FAILED_ID, type Api, type MoveOutcome } from "./api";

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

export interface RunOrganizeDeps {
  root: string;
  files: SortItem[];
  api: Pick<Api, "beginOrganize" | "moveFiles">;
  /**
   * De ids die echt vliegen. Geef dezelfde set als waarmee de AnimFiles gebouwd zijn
   * (`pickAnimated` op dezelfde lijst); ontbreekt ze, dan wordt ze hier berekend.
   */
  animated?: Set<number>;
  /** De klok waarop het schema staat: `() => performance.now()`, zonder `nowOffset`. */
  now: () => number;
  onSchedule(launchAt: Map<number, number>, animated: Set<number>): void;
  onOutcome(o: Outcome): void;
  /** Bestanden per aanvraag aan de Rust-kant (40). */
  batchSize?: number;
  /** Afstand tussen twee lanceringen in ms (12). */
  stagger?: number;
  /** Extra vertraging per tegel, normaal `flight.jitter` van het bestand. */
  jitter?: (id: number) => number;
  /** Wachten; injecteerbaar voor tests. */
  sleep?: (ms: number) => Promise<void>;
  /** Hoeveel ms voor het vertrek van de eerste tegel een batch aangevraagd wordt (400). */
  lead?: number;
  /** Hoeveel ms na nu de eerste tegel vertrekt (250). */
  startDelay?: number;
}

export interface RunOrganizeResult {
  moved: number;
  failed: Outcome[];
  /** Gezet als de Rust-kant meldt dat het ongedaan-maken-log niet bewaard kon worden; geen bestand. */
  logError?: string;
}

const NO_OUTCOME = copy.noOutcome;

const defaultSleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

function toOutcome(o: MoveOutcome): Outcome {
  return { id: o.id, ok: o.ok, dest: o.dest ?? undefined, error: o.error ?? undefined };
}

function errorText(err: unknown): string {
  if (err instanceof Error) return err.message;
  return typeof err === "string" ? err : copy.moveFailed;
}

/**
 * Stuurt het opruimen aan: beginOrganize, dan het lanceerschema voor de geanimeerde tegels
 * (hoogstens 600, buitenste ring eerst), dan de verplaatsingen in batches in lanceervolgorde.
 * Batch k wordt aangevraagd zodra de eerste tegel ervan over `lead` ms vertrekt; de batches
 * zonder animatie volgen meteen daarna. Elke uitkomst gaat direct naar `onOutcome`, ook die
 * van bestanden die stil mee verhuizen. Een geweigerde batch wordt per bestand een mislukking.
 */
export async function runOrganize(deps: RunOrganizeDeps): Promise<RunOrganizeResult> {
  const {
    root,
    files,
    api,
    now,
    onSchedule,
    onOutcome,
    batchSize = 40,
    stagger = 12,
    jitter = () => 0,
    sleep = defaultSleep,
    lead = 400,
    startDelay = 250,
  } = deps;

  const plan = await api.beginOrganize(root);
  const planned = new Set(plan.moves.map((m) => m.id));
  const order = launchOrder(files).filter((id) => planned.has(id));
  let animated = deps.animated ?? pickAnimated(files);
  if ([...animated].some((id) => !planned.has(id))) {
    animated = new Set([...animated].filter((id) => planned.has(id)));
  }
  const animatedOrder = order.filter((id) => animated.has(id));
  const silentOrder = order.filter((id) => !animated.has(id));

  const launchAt = schedule(animatedOrder, now() + startDelay, stagger, jitter);
  onSchedule(launchAt, animated);

  const result: RunOrganizeResult = { moved: 0, failed: [] };
  const take = (o: Outcome) => {
    if (o.ok) result.moved++;
    else result.failed.push(o);
    onOutcome(o);
  };

  const pending: Promise<void>[] = [];
  for (const batch of batches([...animatedOrder, ...silentOrder], batchSize)) {
    const at = launchAt.get(batch[0]);
    if (at !== undefined) {
      const wait = at - lead - now();
      if (wait > 0) await sleep(wait);
    }
    pending.push(
      api.moveFiles(root, batch).then(
        (outs) => {
          const seen = new Set<number>();
          for (const o of outs) {
            if (o.id === LOG_SAVE_FAILED_ID) {
              result.logError = o.error ?? "het log kon niet bewaard worden";
              continue;
            }
            seen.add(o.id);
            take(toOutcome(o));
          }
          for (const id of batch) if (!seen.has(id)) take({ id, ok: false, error: NO_OUTCOME });
        },
        (err: unknown) => {
          const error = errorText(err);
          batch.forEach((id) => take({ id, ok: false, error }));
        },
      ),
    );
  }
  await Promise.all(pending);
  return result;
}
