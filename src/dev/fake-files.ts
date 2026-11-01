// Nepbestanden voor het ontwikkelharnas: een verdeling zoals een echte Downloads-map.
import type { Category, CategoryInfo } from "../lib/api";
import { makeRng } from "../lib/rng";

/** Dezelfde tabel als `get_categories` aan de Rust-kant, voor gebruik zonder Tauri. */
export const FAKE_CATEGORIES: CategoryInfo[] = (
  [
    ["fotos", "Foto's", "afbeeldingen", "Afbeeldingen"],
    ["screenshots", "Screenshots", "afbeeldingen", "Afbeeldingen"],
    ["gifs", "GIF's", "afbeeldingen", "Afbeeldingen"],
    ["design", "Design", "afbeeldingen", "Afbeeldingen"],
    ["videos", "Video's", "videos", "Video's"],
    ["schermopnames", "Schermopnames", "videos", "Video's"],
    ["pdfs", "PDF's", "documenten", "Documenten"],
    ["documenten", "Documenten", "documenten", "Documenten"],
    ["rekenbladen", "Rekenbladen", "documenten", "Documenten"],
    ["presentaties", "Presentaties", "documenten", "Documenten"],
    ["audio", "Audio", "overige", "Overige"],
    ["archieven", "Archieven", "overige", "Overige"],
    ["installers", "Installers", "overige", "Overige"],
    ["code", "Code", "overige", "Overige"],
    ["overig", "Overig", "overige", "Overige"],
    ["duplicaten", "Duplicaten", "dubbel", "Dubbel"],
  ] as const
).map(([id, folder, group, group_label], order) => ({ id, label: folder, folder, group, group_label, order }));

export interface FakeFile {
  id: number;
  name: string;
  category: Category;
  /** Index van de nepminiatuur, enkel bij een deel van de foto's. */
  thumbSeed?: number;
}

/** Gewichten per categorie, ongeveer zoals een Downloads-map van een maker van video's. */
const WEIGHTS: Array<[Category, number]> = [
  ["fotos", 430],
  ["screenshots", 120],
  ["gifs", 25],
  ["design", 15],
  ["videos", 70],
  ["schermopnames", 35],
  ["pdfs", 70],
  ["documenten", 60],
  ["rekenbladen", 22],
  ["presentaties", 12],
  ["audio", 25],
  ["archieven", 28],
  ["installers", 18],
  ["code", 20],
  ["overig", 20],
  ["duplicaten", 60],
];

const NAMES: Record<Category, (n: number, i: number) => string> = {
  fotos: (n) => `IMG_${n}.jpg`,
  screenshots: (_n, i) => `Schermafbeelding 2026-10-${1 + (i % 28)} ${String(8 + (i % 12)).padStart(2, "0")}${String(i % 60).padStart(2, "0")}.png`,
  gifs: (n) => `reactie-${n}.gif`,
  design: (n) => `ontwerp-${n}.fig`,
  videos: (n) => `IMG_${n}.mov`,
  schermopnames: (_n, i) => `Schermopname ${1 + (i % 20)}.mp4`,
  pdfs: (n, i) => `${["factuur", "offerte", "contract", "handleiding", "ticket"][i % 5]}-${n}.pdf`,
  documenten: (n, i) => `${["notities", "brief", "verslag", "script"][i % 4]}-${n}.docx`,
  rekenbladen: (n) => `begroting-${n}.xlsx`,
  presentaties: (n) => `presentatie-${n}.pptx`,
  audio: (n) => `opname-${n}.mp3`,
  archieven: (n) => `archief-${n}.zip`,
  installers: (n) => `Setup-${n}.exe`,
  code: (n) => `script-${n}.ts`,
  overig: (n, i) => `bestand-${n}.${["bin", "dat", "log", "bak"][i % 4]}`,
  duplicaten: (n) => `IMG_${n} (1).jpg`,
};

/**
 * Bouwt `n` nepbestanden met een vaste seed. Elke categorie krijgt minstens één bestand
 * (vanaf 16 bestanden), de rest volgt de gewichten. Ongeveer de helft van de foto's krijgt
 * een `thumbSeed` voor een gekleurde nepminiatuur.
 */
export function makeFakeFiles(n: number, seed = 1): FakeFile[] {
  const rnd = makeRng(seed);
  const total = WEIGHTS.reduce((sum, [, w]) => sum + w, 0);
  const files: FakeFile[] = [];
  const perCat = new Map<Category, number>();
  const push = (category: Category) => {
    const i = perCat.get(category) ?? 0;
    perCat.set(category, i + 1);
    const id = files.length;
    const stamp = 1000 + ((id * 37 + seed * 11) % 9000);
    const file: FakeFile = { id, name: NAMES[category](stamp, i), category };
    if (category === "fotos" && rnd() < 0.55) file.thumbSeed = Math.floor(rnd() * 1000);
    files.push(file);
  };
  if (n >= WEIGHTS.length) for (const [category] of WEIGHTS) push(category);
  while (files.length < n) {
    let pick = rnd() * total;
    for (const [category, weight] of WEIGHTS) {
      pick -= weight;
      if (pick <= 0) {
        push(category);
        break;
      }
    }
  }
  return files;
}

/** Een gekleurd vlak met een lichtere hoek als data-url, zoals een foto-miniatuur. Enkel in de browser. */
export function fakeThumb(seed: number, px = 96): string {
  const rnd = makeRng(seed);
  const c = document.createElement("canvas");
  c.width = c.height = px;
  const g = c.getContext("2d")!;
  const hue = Math.floor(rnd() * 360);
  g.fillStyle = `hsl(${hue} ${35 + rnd() * 30}% ${40 + rnd() * 25}%)`;
  g.fillRect(0, 0, px, px);
  g.fillStyle = `hsl(${(hue + 30) % 360} 40% ${70 + rnd() * 15}%)`;
  const w = px * (0.3 + rnd() * 0.4);
  g.fillRect(rnd() * (px - w), rnd() * (px - w), w, w * (0.5 + rnd()));
  g.fillStyle = "rgba(18,21,15,.25)";
  g.fillRect(0, px * (0.7 + rnd() * 0.2), px, px);
  return c.toDataURL("image/png");
}
