export type Pt = { x: number; y: number };

export interface FlightParams {
  /** Vluchtduur in ms (760 tot 1060). */
  dur: number;
  /** Hoogte van de boog in px (40 tot 120). */
  lift: number;
  /** Extra vertraging bij het lanceren in ms (0 tot 90). */
  jitter: number;
}

export function flightParams(rnd: () => number): FlightParams {
  const dur = 760 + rnd() * 300;
  const lift = 40 + rnd() * 80;
  const jitter = rnd() * 90;
  return { dur, lift, jitter };
}

export const easeInOutCubic = (p: number): number =>
  p < 0.5 ? 4 * p * p * p : 1 - Math.pow(-2 * p + 2, 3) / 2;

/** Punt op de kwadratische bezier van `from` naar `to`; controlepunt = midden met y = min(from.y, to.y) - lift. */
export function flightPoint(from: Pt, to: Pt, lift: number, e: number): Pt {
  const cx = (from.x + to.x) / 2;
  const cy = Math.min(from.y, to.y) - lift;
  const u = 1 - e;
  return {
    x: u * u * from.x + 2 * u * e * cx + e * e * to.x,
    y: u * u * from.y + 2 * u * e * cy + e * e * to.y,
  };
}

/**
 * Toestand van een vliegende tegel bij voortgang p (0..1). `speed` is de afstand
 * tot het punt bij p - 0.016, (vx, vy) de eenheidsrichting (0, 0 bij stilstand).
 */
export function flightState(
  from: Pt,
  to: Pt,
  p: number,
  rot0: number,
  scaleTo: number,
  lift: number,
): { x: number; y: number; rot: number; scale: number; speed: number; vx: number; vy: number } {
  const q = Math.min(1, Math.max(0, p));
  const e = easeInOutCubic(q);
  const { x, y } = flightPoint(from, to, lift, e);
  const prev = flightPoint(from, to, lift, easeInOutCubic(Math.max(0, q - 0.016)));
  const speed = Math.hypot(x - prev.x, y - prev.y);
  return {
    x,
    y,
    rot: rot0 * (1 - e),
    scale: 1 + (scaleTo - 1) * e,
    speed,
    vx: speed ? (x - prev.x) / speed : 0,
    vy: speed ? (y - prev.y) / speed : 0,
  };
}
