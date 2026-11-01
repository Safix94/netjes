// Van een scanresultaat naar de hoop: welke bestanden vliegen (hoogstens 600), waar ze liggen,
// hoe ze vliegen en in welk vakje ze landen. App bouwt dit één keer per scan.
import type { AnimFile } from "../components/FlyCanvas";
import type { FileEntry } from "./api";
import { flightParams } from "./flight";
import { layoutPile } from "./pile";
import { makeRng } from "./rng";
import { launchOrder, pickAnimated, type SortItem } from "./sorter";

export interface AnimModel {
  /** De tegels op het canvas, hoogstens 600. */
  anim: AnimFile[];
  /** Dezelfde ids als in `anim`; geef deze set aan `runOrganize`. */
  animated: Set<number>;
  /** Alle bestanden voor `runOrganize`; stille bestanden hebben radius -1 en vertrekken als laatste. */
  items: SortItem[];
}

export const MAX_TILES = 600;

export function buildAnimModel(files: FileEntry[], radius: number, seed: number): AnimModel {
  const base: SortItem[] = files.map((f) => ({ id: f.id, category: f.category, radius: -1 }));
  const animated = pickAnimated(base, MAX_TILES);
  const chosen = files.filter((f) => animated.has(f.id));
  const pile = layoutPile(chosen.length, radius, seed ^ chosen.length);
  const rnd = makeRng(seed + 1);
  const byId = new Map<number, AnimFile>();
  chosen.forEach((f, i) => {
    byId.set(f.id, { id: f.id, category: f.category, pile: pile[i], flight: flightParams(rnd), slot: 0, state: "pile" });
  });
  const items = base.map((item) => ({ ...item, radius: byId.get(item.id)?.pile.radius ?? -1 }));
  // Vakjes in lanceervolgorde: wie eerst landt, krijgt vakje 0.
  const perCategory = new Map<string, number>();
  for (const id of launchOrder(items)) {
    const tile = byId.get(id);
    if (!tile) continue;
    tile.slot = perCategory.get(tile.category) ?? 0;
    perCategory.set(tile.category, tile.slot + 1);
  }
  return { anim: [...byId.values()], animated, items };
}

/** Ids van de tegels met de buitenste ring eerst, voor het laden van miniaturen. */
export function thumbOrder(anim: AnimFile[]): number[] {
  return [...anim].sort((a, b) => b.pile.radius - a.pile.radius || a.id - b.id).map((a) => a.id);
}

/** Knipt een lijst in stukken van `size`. */
export function chunk<T>(list: T[], size: number): T[][] {
  const out: T[][] = [];
  for (let i = 0; i < list.length; i += size) out.push(list.slice(i, i + size));
  return out;
}
