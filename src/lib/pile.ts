import { makeRng } from "./rng";

/** Een tegel in de donut-hoop, relatief aan het middelpunt (0, 0). */
export interface PilePos {
  x: number;
  y: number;
  radius: number;
  rot: number;
  size: number;
  phase: number;
}

const GOLDEN_ANGLE = Math.PI * (3 - Math.sqrt(5));

/** Tegelgrootte in px: krimpt met het aantal bestanden, tussen 16 en 60. */
export function tileSize(n: number): number {
  return Math.min(60, Math.max(16, 60 - 0.09 * n));
}

/**
 * Phyllotaxis-hoop met een gat in het midden. Tegel i ligt op afstand
 * radius * sqrt(.15 + .85 * i / n) van het midden, gedraaid met de gulden hoek.
 * Deterministisch voor dezelfde seed.
 */
export function layoutPile(n: number, radius: number, seed: number): PilePos[] {
  const rnd = makeRng(seed);
  const size = tileSize(n);
  const pile: PilePos[] = [];
  for (let i = 0; i < n; i++) {
    const r = radius * Math.sqrt(0.15 + (0.85 * i) / n) * (0.9 + rnd() * 0.2);
    const a = i * GOLDEN_ANGLE + rnd() * 0.35;
    const rot = -25 + rnd() * 50;
    const tile = size * (0.85 + 0.3 * rnd());
    const phase = rnd() * 2 * Math.PI;
    pile.push({ x: Math.cos(a) * r, y: Math.sin(a) * r, radius: r, rot, size: tile, phase });
  }
  return pile;
}
