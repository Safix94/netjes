// De toestand van de animatie, los van het canvas: welke tegel waar staat op tijdstip `now`,
// wanneer er geland of teruggevlogen wordt en wanneer de finale begint. FlyCanvas tekent enkel
// wat `frame` teruggeeft, zodat dit gedrag zonder browser testbaar blijft.
import { easeInOutCubic, flightPoint, flightState, type FlightParams, type Pt } from "./flight";
import type { PilePos } from "./pile";
import { landingFor, pickAnimated, type Outcome } from "./sorter";

export type TileState = "pile" | "flying" | "landed" | "returning";

export interface SceneFile {
  id: number;
  category: string;
  thumb?: string;
  pile: PilePos;
  flight: FlightParams;
  launchAt?: number;
  slot: number;
  state: TileState;
}

/** Doel van een vlucht in canvascoördinaten: middelpunt en zijde van het vakje. */
export interface SlotTarget {
  x: number;
  y: number;
  s: number;
}

export type PoseMode = "pile" | "fly" | "hold" | "return";

export interface Pose {
  id: number;
  mode: PoseMode;
  x: number;
  y: number;
  rot: number;
  scale: number;
  speed: number;
  vx: number;
  vy: number;
}

/** Een gelande tegel die stil in zijn vakje ligt. */
export interface Landed {
  id: number;
  x: number;
  y: number;
  s: number;
}

export interface FrameResult {
  poses: Pose[];
  landed: Landed[];
  finaleAt: number | undefined;
}

export interface SceneEvents {
  onLand(id: number): void;
  onReturn(id: number): void;
}

export const MAX_ANIMATED = 600;
export const SLOTS_PER_CARD = 12;
/** Zonder uitkomst blijft een tegel hangen op dit punt van de vlucht. */
export const HOLD_P = 0.98;
/** Duur van de terugvlucht naar de hoop na een mislukte verplaatsing. */
export const RETURN_MS = 600;
/** Zonder vakje landt een tegel hier onder het middelpunt van de hoop. */
const FALLBACK_DROP = { dy: 260, s: 12 };

interface Runtime {
  landed: boolean;
  landedInSlot: boolean;
  returned: boolean;
  returnAt?: number;
  dest?: SlotTarget;
  realTarget: boolean;
  pose: Pose;
}

export class Scene {
  private ordered: SceneFile[] = [];
  private runtime = new Map<number, Runtime>();
  private finale: number | undefined;
  private readonly poses: Pose[] = [];
  private readonly landedList: Landed[] = [];

  constructor(private readonly events: SceneEvents) {}

  /** De tegels die echt getekend worden: hoogstens 600, buitenste ring eerst. */
  get files(): readonly SceneFile[] {
    return this.ordered;
  }

  get finaleAt(): number | undefined {
    return this.finale;
  }

  setFiles(files: SceneFile[]): void {
    let chosen = files;
    if (files.length > MAX_ANIMATED) {
      const picked = pickAnimated(
        files.map((f) => ({ id: f.id, category: f.category, radius: f.pile.radius })),
        MAX_ANIMATED,
      );
      chosen = files.filter((f) => picked.has(f.id));
    }
    this.ordered = [...chosen].sort((a, b) => b.pile.radius - a.pile.radius || a.id - b.id);
    const ids = new Set(this.ordered.map((f) => f.id));
    for (const id of this.runtime.keys()) if (!ids.has(id)) this.runtime.delete(id);
  }

  /** Vergeet landingen, terugvluchten en finale; de hoop staat weer vol. */
  reset(): void {
    this.runtime.clear();
    this.finale = undefined;
  }

  /** Na een resize: de doelen opnieuw opvragen. */
  invalidateTargets(): void {
    for (const rt of this.runtime.values()) rt.dest = undefined;
  }

  frame(
    now: number,
    center: Pt,
    sorting: boolean,
    outcomes: ReadonlyMap<number, Outcome>,
    slotFor: (category: string, slot: number) => SlotTarget | null,
  ): FrameResult {
    this.poses.length = 0;
    this.landedList.length = 0;
    let launched = 0;
    let settled = 0;

    for (const file of this.ordered) {
      const rt = this.runtimeFor(file);
      if (file.state === "landed" && !rt.landed) {
        rt.landed = true;
        rt.landedInSlot = file.slot < SLOTS_PER_CARD;
      }
      const scheduled = sorting && file.launchAt !== undefined;
      if (scheduled) launched++;

      if (rt.landed) {
        settled++;
        if (rt.landedInSlot) {
          const dest = this.target(file, rt, center, slotFor);
          if (rt.realTarget) this.landedList.push({ id: file.id, x: dest.x, y: dest.y, s: dest.s });
        }
        continue;
      }

      const from = { x: center.x + file.pile.x, y: center.y + file.pile.y };
      const pose = rt.pose;

      if (rt.returnAt !== undefined) {
        const q = (now - rt.returnAt) / RETURN_MS;
        if (q >= 1) {
          rt.returnAt = undefined;
          rt.returned = true;
          this.events.onReturn(file.id);
        } else {
          const dest = this.target(file, rt, center, slotFor);
          const scaleStart = dest.s / file.pile.size;
          const st = flightState(dest, from, Math.max(0, q), 0, 1, file.flight.lift);
          const e = easeInOutCubic(Math.max(0, q));
          this.setPose(pose, "return", st.x, st.y, file.pile.rot * e, scaleStart + (1 - scaleStart) * e, st.speed, st.vx, st.vy);
          this.poses.push(pose);
          continue;
        }
      }

      if (rt.returned) settled++;

      if (!scheduled || rt.returned || now < file.launchAt!) {
        const bob = sorting ? 0 : Math.sin(now / 340 + file.pile.phase) * (1 + (file.id % 3) * 0.3);
        this.setPose(pose, "pile", from.x, from.y + bob, file.pile.rot, 1, 0, 0, 0);
        this.poses.push(pose);
        continue;
      }

      const p = (now - file.launchAt!) / file.flight.dur;
      const dest = this.target(file, rt, center, slotFor);
      const scaleTo = dest.s / file.pile.size;
      const landing = landingFor(outcomes.get(file.id), p);
      if (landing === "fly") {
        const st = flightState(from, dest, p, file.pile.rot, scaleTo, file.flight.lift);
        this.setPose(pose, "fly", st.x, st.y, st.rot, st.scale, st.speed, st.vx, st.vy);
        this.poses.push(pose);
      } else if (landing === "hold") {
        const st = flightState(from, dest, HOLD_P, file.pile.rot, scaleTo, file.flight.lift);
        this.setPose(pose, "hold", st.x, st.y, st.rot, st.scale, 0, 0, 0);
        this.poses.push(pose);
      } else if (landing === "land") {
        rt.landed = true;
        rt.landedInSlot = file.slot < SLOTS_PER_CARD;
        settled++;
        if (rt.landedInSlot && rt.realTarget) {
          this.landedList.push({ id: file.id, x: dest.x, y: dest.y, s: dest.s });
        }
        this.events.onLand(file.id);
      } else {
        rt.returnAt = now;
        const at = flightPoint(from, dest, file.flight.lift, 1);
        this.setPose(pose, "return", at.x, at.y, 0, scaleTo, 0, 0, 0);
        this.poses.push(pose);
      }
    }

    if (sorting && this.finale === undefined && launched > 0 && settled >= launched) {
      this.finale = now;
    }
    return { poses: this.poses, landed: this.landedList, finaleAt: this.finale };
  }

  private runtimeFor(file: SceneFile): Runtime {
    let rt = this.runtime.get(file.id);
    if (!rt) {
      rt = {
        landed: false,
        landedInSlot: false,
        returned: false,
        realTarget: false,
        pose: { id: file.id, mode: "pile", x: 0, y: 0, rot: 0, scale: 1, speed: 0, vx: 0, vy: 0 },
      };
      this.runtime.set(file.id, rt);
    }
    return rt;
  }

  private target(
    file: SceneFile,
    rt: Runtime,
    center: Pt,
    slotFor: (category: string, slot: number) => SlotTarget | null,
  ): SlotTarget {
    if (rt.dest) return rt.dest;
    const found = slotFor(file.category, file.slot % SLOTS_PER_CARD);
    rt.realTarget = found !== null;
    rt.dest = found ?? { x: center.x, y: center.y + FALLBACK_DROP.dy, s: FALLBACK_DROP.s };
    return rt.dest;
  }

  private setPose(
    pose: Pose,
    mode: PoseMode,
    x: number,
    y: number,
    rot: number,
    scale: number,
    speed: number,
    vx: number,
    vy: number,
  ): void {
    pose.mode = mode;
    pose.x = x;
    pose.y = y;
    pose.rot = rot;
    pose.scale = scale;
    pose.speed = speed;
    pose.vx = vx;
    pose.vy = vy;
  }
}
