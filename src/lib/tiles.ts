// Getekende tegels per categorie en de sprite-cache. Alles wat duur is (rand, schaduw,
// afronding) wordt één keer in een sprite gebakken; de animatielus tekent enkel drawImage.

/** Alle categorie-ids uit de spec-tabel, in de volgorde van de tabel. */
export const CATEGORIES = [
  "fotos",
  "screenshots",
  "gifs",
  "design",
  "videos",
  "schermopnames",
  "pdfs",
  "documenten",
  "rekenbladen",
  "presentaties",
  "audio",
  "archieven",
  "installers",
  "code",
  "overig",
  "duplicaten",
] as const;

/** Categorieën die een paper-vlak tonen tot de echte miniatuur er is. */
export const THUMB_CATEGORIES = new Set<string>(["fotos", "screenshots", "gifs"]);

export interface Palette {
  ink: string;
  body: string;
  muted: string;
  paper: string;
  surface: string;
  border: string;
  lime: string;
  limeSoft: string;
}

const FALLBACK: Palette = {
  ink: "#12150F",
  body: "#3F4640",
  muted: "#6B736A",
  paper: "#F4F6EE",
  surface: "#FFFFFF",
  border: "#E3E7DB",
  lime: "#CDF464",
  limeSoft: "#E6FAB4",
};

const VARS: Record<keyof Palette, string> = {
  ink: "--ink",
  body: "--body",
  muted: "--muted",
  paper: "--paper",
  surface: "--surface",
  border: "--border",
  lime: "--lime",
  limeSoft: "--lime-soft",
};

/** Leest de huisstijlkleuren uit de css-variabelen op `el`; ontbreekt een variabele, dan de vaste hexwaarde. */
export function readPalette(el: Element = document.documentElement): Palette {
  const style = getComputedStyle(el);
  const out = { ...FALLBACK };
  for (const key of Object.keys(VARS) as (keyof Palette)[]) {
    const value = style.getPropertyValue(VARS[key]).trim();
    if (value) out[key] = value;
  }
  return out;
}

const EXT: Record<string, string> = {
  design: "FIG",
  videos: "MP4",
  schermopnames: "MOV",
  pdfs: "PDF",
  documenten: "DOCX",
  rekenbladen: "XLSX",
  presentaties: "PPTX",
  audio: "MP3",
  archieven: "ZIP",
  installers: "EXE",
  code: "TS",
};

const DARK = new Set(["videos", "schermopnames", "installers", "code"]);

const MONO = '500 12px "JetBrains Mono", Consolas, monospace';

function docLines(g: CanvasRenderingContext2D, color: string, seed: number): void {
  g.fillStyle = color;
  const widths = [84, 92, 70, 88, 60, 80];
  const shift = seed % 3;
  widths.forEach((w, i) => {
    g.fillRect(18, 30 + i * 13, widths[(i + shift) % widths.length] * 0.2 + w * 0.8, 5);
  });
}

/**
 * Tekent de tegel van een categorie op (0, 0) met zijde `size`. Het ontwerp volgt de mockup;
 * de coördinaten zijn in een vlak van 128 en worden geschaald. Foto's, screenshots en gifs
 * krijgen enkel een paper-vlak: hun miniatuur komt later via de sprite-cache.
 */
export function drawTypeTile(
  ctx: CanvasRenderingContext2D,
  category: string,
  seed: number,
  size: number,
  palette: Palette = readPalette(),
): void {
  const { ink, paper, border: line, muted, limeSoft, surface } = palette;
  const dark = DARK.has(category);
  ctx.save();
  ctx.scale(size / 128, size / 128);
  ctx.fillStyle = dark ? ink : surface;
  ctx.fillRect(0, 0, 128, 128);
  switch (category) {
    case "fotos":
    case "screenshots":
    case "gifs":
      ctx.fillStyle = paper;
      ctx.fillRect(0, 0, 128, 128);
      break;
    case "videos":
    case "schermopnames":
      ctx.fillStyle = surface;
      ctx.beginPath();
      ctx.moveTo(50, 40);
      ctx.lineTo(88, 64);
      ctx.lineTo(50, 88);
      ctx.closePath();
      ctx.fill();
      break;
    case "code":
      ctx.fillStyle = surface;
      ctx.font = '500 44px "JetBrains Mono", Consolas, monospace';
      ctx.fillText("{ }", 28, 80);
      break;
    case "installers":
      ctx.strokeStyle = surface;
      ctx.lineWidth = 4;
      ctx.strokeRect(34, 34, 60, 60);
      ctx.fillStyle = surface;
      ctx.fillRect(34, 34, 60, 14);
      break;
    case "audio":
      ctx.fillStyle = ink;
      for (let i = 0; i < 9; i++) {
        const h = 14 + Math.abs(Math.sin(i * 1.7 + seed)) * 50;
        ctx.fillRect(20 + i * 10, 64 - h / 2, 6, h);
      }
      break;
    case "archieven":
      ctx.fillStyle = paper;
      ctx.fillRect(0, 0, 128, 128);
      ctx.fillStyle = line;
      for (let y = 10; y < 128; y += 16) ctx.fillRect(56, y, 16, 8);
      break;
    case "rekenbladen":
      ctx.strokeStyle = line;
      ctx.lineWidth = 2;
      for (let i = 1; i < 5; i++) {
        ctx.beginPath();
        ctx.moveTo(0, i * 25.6);
        ctx.lineTo(128, i * 25.6);
        ctx.stroke();
        ctx.beginPath();
        ctx.moveTo(i * 32, 0);
        ctx.lineTo(i * 32, 128);
        ctx.stroke();
      }
      ctx.fillStyle = limeSoft;
      ctx.fillRect(0, 0, 128, 25);
      break;
    case "presentaties":
      ctx.fillStyle = paper;
      ctx.fillRect(0, 0, 128, 128);
      ctx.fillStyle = ink;
      ctx.fillRect(18, 34, 92, 10);
      ctx.fillStyle = line;
      ctx.fillRect(18, 56, 70, 6);
      ctx.fillRect(18, 70, 80, 6);
      break;
    case "design":
      ctx.fillStyle = paper;
      ctx.fillRect(0, 0, 128, 128);
      ctx.fillStyle = ink;
      ctx.beginPath();
      ctx.arc(50, 56, 22, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = line;
      ctx.fillRect(60, 60, 44, 44);
      break;
    case "duplicaten":
      ctx.fillStyle = paper;
      ctx.fillRect(0, 0, 128, 128);
      ctx.fillStyle = line;
      ctx.fillRect(44, 44, 56, 56);
      ctx.fillStyle = surface;
      ctx.fillRect(28, 28, 56, 56);
      ctx.strokeStyle = ink;
      ctx.lineWidth = 3;
      ctx.strokeRect(28, 28, 56, 56);
      break;
    case "pdfs":
      docLines(ctx, line, seed);
      ctx.fillStyle = ink;
      ctx.fillRect(18, 14, 36, 8);
      break;
    default:
      // documenten, overig en onbekende categorieën: tekstregels
      docLines(ctx, line, seed);
      break;
  }
  const ext = EXT[category];
  if (ext) {
    ctx.fillStyle = dark ? "rgba(255,255,255,.55)" : muted;
    ctx.font = MONO;
    ctx.fillText(ext, 10, 120);
  }
  ctx.restore();
}

/** Sleutel voor de sprite-cache: zonder miniatuur delen bestanden van één categorie drie varianten. */
export function spriteKey(file: { id: number; category: string; thumb?: string }): string {
  return file.thumb ? `${file.category}|t|${file.thumb}` : `${file.category}|${file.id % 3}`;
}

export function roundedRect(
  g: CanvasRenderingContext2D,
  x: number,
  y: number,
  w: number,
  h: number,
  r: number,
): void {
  g.beginPath();
  g.moveTo(x + r, y);
  g.arcTo(x + w, y, x + w, y + h, r);
  g.arcTo(x + w, y + h, x, y + h, r);
  g.arcTo(x, y + h, x, y, r);
  g.arcTo(x, y, x + w, y, r);
  g.closePath();
}

export type SpriteStyle = "card" | "flat";

/**
 * Cache van gebakken sprites. Stijl `card` (standaard): een vierkant van `size` px met een
 * witte rand van 8/80 en een zachte schaduw, zoals de tegels in de donut. Stijl `flat`:
 * enkel de inhoud met afgeronde hoeken, voor de vakjes op de kaarten.
 * `scale` vermenigvuldigt de pixelmaat (devicePixelRatio) zonder de logische maat te wijzigen.
 */
export class SpriteCache {
  private readonly sprites = new Map<string, HTMLCanvasElement>();

  constructor(
    public readonly size = 80,
    public readonly scale = 1,
    public readonly style: SpriteStyle = "card",
    private readonly palette: Palette = readPalette(),
  ) {}

  /** Geeft de sprite voor `key`; bestaat ze nog niet, dan tekent `draw` de inhoud op een scratch-canvas. */
  get(key: string, draw: (content: HTMLCanvasElement) => void): HTMLCanvasElement {
    const hit = this.sprites.get(key);
    if (hit) return hit;
    const px = Math.round(this.size * this.scale);
    const sprite = document.createElement("canvas");
    sprite.width = sprite.height = px;
    const g = sprite.getContext("2d")!;
    const content = document.createElement("canvas");
    if (this.style === "card") {
      const pad = (8 / 80) * px;
      const edge = (2 / 80) * px;
      const inner = px - 2 * pad - 2 * edge;
      content.width = content.height = Math.max(1, Math.round(inner));
      draw(content);
      g.shadowColor = "rgba(18,21,15,.28)";
      g.shadowBlur = (6 / 80) * px;
      g.shadowOffsetY = (2 / 80) * px;
      g.fillStyle = this.palette.surface;
      roundedRect(g, pad, pad, px - 2 * pad, px - 2 * pad, (8 / 80) * px);
      g.fill();
      g.shadowColor = "transparent";
      g.shadowBlur = 0;
      g.shadowOffsetY = 0;
      g.save();
      roundedRect(g, pad + edge, pad + edge, inner, inner, (6 / 80) * px);
      g.clip();
      g.drawImage(content, pad + edge, pad + edge, inner, inner);
      g.restore();
    } else {
      content.width = content.height = px;
      draw(content);
      g.save();
      roundedRect(g, 0, 0, px, px, (3 / 40) * px);
      g.clip();
      g.drawImage(content, 0, 0, px, px);
      g.restore();
    }
    this.sprites.set(key, sprite);
    return sprite;
  }

  has(key: string): boolean {
    return this.sprites.has(key);
  }

  clear(): void {
    this.sprites.clear();
  }
}

/** Tekent `img` vlakvullend (cover) in het canvas `c`, gecentreerd. */
export function drawCover(c: HTMLCanvasElement, img: HTMLImageElement | HTMLCanvasElement): void {
  const g = c.getContext("2d")!;
  const w = img instanceof HTMLImageElement ? img.naturalWidth : img.width;
  const h = img instanceof HTMLImageElement ? img.naturalHeight : img.height;
  if (!w || !h) return;
  const k = Math.max(c.width / w, c.height / h);
  g.drawImage(img, c.width / 2 - (w * k) / 2, c.height / 2 - (h * k) / 2, w * k, h * k);
}
