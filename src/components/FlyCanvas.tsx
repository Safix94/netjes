// Eén canvas over de hele app: de donut in rust, de vluchten, de gelande tegels in hun vakjes
// en de finale. Het rekenwerk zit in lib/scene.ts; hier wordt enkel getekend vanuit sprites.
import { useEffect, useRef, type CSSProperties, type RefObject } from "react";
import { thumbUrl } from "../lib/api";
import type { FlightParams } from "../lib/flight";
import type { PilePos } from "../lib/pile";
import { Scene, type SlotTarget, type TileState } from "../lib/scene";
import type { Outcome } from "../lib/sorter";
import { SpriteCache, drawCover, drawTypeTile, readPalette, spriteKey, type Palette } from "../lib/tiles";

export type Phase = "idle" | "scanning" | "ready" | "tidy" | "sorting" | "done" | "undoing" | "error";

export interface AnimFile {
  id: number;
  category: string;
  /** Pad in de miniatuurcache (via `thumbUrl`) of een data-/blob-url. */
  thumb?: string;
  pile: PilePos;
  flight: FlightParams;
  launchAt?: number;
  slot: number;
  state: TileState;
}

export interface FlyCanvasProps {
  /**
   * Enkel de tegels die echt vliegen: de ids uit `pickAnimated` op de scanlijst, met een
   * `pile` uit `layoutPile(files.length, ...)`. Geef dezelfde set als `animated` aan
   * `runOrganize`, anders krijgen getekende tegels geen `launchAt`. Meer dan 600 wordt hier
   * alsnog afgekapt, maar dan klopt de hoop niet meer met het schema.
   */
  files: AnimFile[];
  phase: Phase;
  /** Het vierkant waarin de donut ligt; het middelpunt ervan is het middelpunt van de hoop. */
  stageRef: RefObject<HTMLDivElement | null>;
  /** Het vakje `i` (0 tot 11) op de kaart van `cat`, in viewportcoördinaten. */
  getSlotRect(cat: string, i: number): DOMRect | null;
  /** Wordt bij `performance.now()` opgeteld; handig voor stilstaande opnames. */
  nowOffset?: number;
  onLand(id: number): void;
  onReturn(id: number): void;
  outcomes: Map<number, Outcome>;
  /** Start van de finale. Ontbreekt hij, dan start de finale zelf na de laatste landing of terugkeer. */
  finaleAt?: number;
}

const ACTIVE: ReadonlySet<Phase> = new Set<Phase>(["sorting", "done"]);
const SPRITE = 80;
const SLOT_SPRITE = 40;
/** Een tegel van `size` px wordt getekend als een sprite van size * 80 / 64, zoals de mockup. */
const SPRITE_PER_SIZE = SPRITE / 64;
const BLUR_FROM = 10;

const canvasStyle: CSSProperties = {
  position: "absolute",
  inset: 0,
  width: "100%",
  height: "100%",
  pointerEvents: "none",
  zIndex: 30,
  display: "block",
};

type Thumb = { img: HTMLImageElement; ready: boolean };

function resolveThumb(thumb: string): string {
  return /^(data:|blob:|https?:)/.test(thumb) ? thumb : thumbUrl(thumb);
}

function drawFinale(ctx: CanvasRenderingContext2D, cx: number, cy: number, p: number, palette: Palette): void {
  ctx.globalAlpha = 0.14;
  ctx.strokeStyle = palette.ink;
  ctx.setLineDash([3, 9]);
  ctx.lineWidth = 4;
  ctx.beginPath();
  ctx.arc(cx, cy, 72, 0, Math.PI * 2);
  ctx.stroke();
  ctx.setLineDash([]);
  for (let i = 0; i < 8; i++) {
    const e = Math.min(1, Math.max(0, p - i * 0.09) / 0.4);
    const ease = 1 - Math.pow(1 - e, 3);
    const a = (i / 8) * Math.PI * 2 + p * 0.9;
    const rad = 72 + Math.sin(p * 1.6 + i) * 4;
    ctx.globalAlpha = ease;
    ctx.fillStyle = i % 2 ? palette.ink : palette.lime;
    ctx.beginPath();
    ctx.arc(cx + Math.cos(a) * rad, cy + Math.sin(a) * rad, 7 * (0.85 + 0.15 * ease), 0, Math.PI * 2);
    ctx.fill();
    if (i % 2 === 0) {
      ctx.globalAlpha = ease * 0.25;
      ctx.strokeStyle = palette.ink;
      ctx.lineWidth = 1;
      ctx.stroke();
    }
  }
  ctx.globalAlpha = 1;
}

export function FlyCanvas(props: FlyCanvasProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const propsRef = useRef(props);
  propsRef.current = props;

  const sceneRef = useRef<Scene | null>(null);
  if (!sceneRef.current) {
    sceneRef.current = new Scene({
      onLand: (id) => propsRef.current.onLand(id),
      onReturn: (id) => propsRef.current.onReturn(id),
    });
  }
  const scene = sceneRef.current;
  const slotCache = useRef(new Map<string, SlotTarget>());

  useEffect(() => {
    scene.setFiles(props.files);
  }, [scene, props.files]);

  const lastPhase = useRef<Phase | null>(null);
  useEffect(() => {
    const prev = lastPhase.current;
    lastPhase.current = props.phase;
    if (!ACTIVE.has(props.phase) || (prev === "done" && props.phase === "sorting")) {
      scene.reset();
    }
    slotCache.current.clear();
    scene.invalidateTargets();
  }, [scene, props.phase]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const palette = readPalette(canvas);
    let q = Math.min(window.devicePixelRatio || 1, 2);
    let cards = new SpriteCache(SPRITE, q, "card", palette);
    let slots = new SpriteCache(SLOT_SPRITE, q, "flat", palette);
    const thumbs = new Map<string, Thumb>();
    let byId = new Map<number, AnimFile>();
    let indexedFiles: readonly AnimFile[] | null = null;
    let raf = 0;

    const invalidateLayout = () => {
      slotCache.current.clear();
      scene.invalidateTargets();
    };
    const observer = typeof ResizeObserver === "function" ? new ResizeObserver(invalidateLayout) : null;
    observer?.observe(canvas);
    window.addEventListener("resize", invalidateLayout);
    window.addEventListener("scroll", invalidateLayout, true);

    const thumbFor = (thumb: string): HTMLImageElement | undefined => {
      let entry = thumbs.get(thumb);
      if (!entry) {
        const img = new Image();
        img.decoding = "async";
        entry = { img, ready: false };
        thumbs.set(thumb, entry);
        img.onload = () => {
          entry!.ready = img.naturalWidth > 0;
        };
        img.src = resolveThumb(thumb);
      }
      return entry.ready ? entry.img : undefined;
    };

    const drawContent = (file: AnimFile, img: HTMLImageElement | undefined) => (c: HTMLCanvasElement) => {
      if (img) drawCover(c, img);
      else drawTypeTile(c.getContext("2d")!, file.category, file.id, c.width, palette);
    };

    const spriteFor = (cache: SpriteCache, file: AnimFile): HTMLCanvasElement => {
      const img = file.thumb ? thumbFor(file.thumb) : undefined;
      const key = img ? spriteKey(file) : spriteKey({ id: file.id, category: file.category });
      return cache.get(key, drawContent(file, img));
    };

    const frame = () => {
      raf = requestAnimationFrame(frame);
      const { phase, nowOffset, outcomes, stageRef, getSlotRect, finaleAt } = propsRef.current;
      const now = performance.now() + (nowOffset ?? 0);

      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      if (dpr !== q) {
        q = dpr;
        cards = new SpriteCache(SPRITE, q, "card", palette);
        slots = new SpriteCache(SLOT_SPRITE, q, "flat", palette);
      }
      const rect = canvas.getBoundingClientRect();
      const w = Math.round(rect.width * q);
      const h = Math.round(rect.height * q);
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w;
        canvas.height = h;
        invalidateLayout();
      }
      ctx.setTransform(q, 0, 0, q, 0, 0);
      ctx.clearRect(0, 0, rect.width, rect.height);

      if (scene.files !== indexedFiles) {
        indexedFiles = scene.files;
        byId = new Map(indexedFiles.map((f) => [f.id, f]));
      }

      const stage = stageRef.current?.getBoundingClientRect();
      const center = stage
        ? { x: stage.left - rect.left + stage.width / 2, y: stage.top - rect.top + stage.height / 2 }
        : { x: rect.width / 2, y: rect.height / 2 };

      const slotFor = (cat: string, i: number): SlotTarget | null => {
        const key = `${cat}:${i}`;
        const hit = slotCache.current.get(key);
        if (hit) return hit;
        const r = getSlotRect(cat, i);
        if (!r || !r.width) return null;
        const t = { x: r.left - rect.left + r.width / 2, y: r.top - rect.top + r.height / 2, s: r.width };
        slotCache.current.set(key, t);
        return t;
      };

      const active = ACTIVE.has(phase);
      const result = scene.frame(now, center, active, outcomes, slotFor);

      for (const l of result.landed) {
        const file = byId.get(l.id);
        if (!file) continue;
        ctx.drawImage(spriteFor(slots, file), l.x - l.s / 2, l.y - l.s / 2, l.s, l.s);
      }

      for (const pose of result.poses) {
        const file = byId.get(pose.id);
        if (!file) continue;
        const sprite = spriteFor(cards, file);
        const s = file.pile.size * SPRITE_PER_SIZE * pose.scale;
        const a = (pose.rot * Math.PI) / 180;
        const cos = Math.cos(a) * q;
        const sin = Math.sin(a) * q;
        ctx.setTransform(cos, sin, -sin, cos, pose.x * q, pose.y * q);
        if (pose.speed > BLUR_FROM) {
          ctx.globalAlpha = Math.min(0.3, (pose.speed - BLUR_FROM) / 50);
          ctx.drawImage(sprite, -s / 2 - 10 * pose.vx, -s / 2 - 10 * pose.vy, s, s);
          ctx.drawImage(sprite, -s / 2 - 5 * pose.vx, -s / 2 - 5 * pose.vy, s, s);
          ctx.globalAlpha = 1;
        }
        ctx.drawImage(sprite, -s / 2, -s / 2, s, s);
      }
      ctx.setTransform(q, 0, 0, q, 0, 0);

      const finaleStart = finaleAt ?? result.finaleAt;
      if (active && finaleStart !== undefined && now >= finaleStart) {
        drawFinale(ctx, center.x, center.y, (now - finaleStart) / 1000, palette);
      }
    };

    raf = requestAnimationFrame(frame);
    return () => {
      cancelAnimationFrame(raf);
      observer?.disconnect();
      window.removeEventListener("resize", invalidateLayout);
      window.removeEventListener("scroll", invalidateLayout, true);
      cards.clear();
      slots.clear();
    };
  }, [scene]);

  return <canvas ref={canvasRef} className="fly-canvas" style={canvasStyle} aria-hidden="true" />;
}

export default FlyCanvas;
