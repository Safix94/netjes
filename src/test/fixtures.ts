import type { Category, CategoryInfo, FileEntry, Group } from "../lib/api";

const row = (id: Category, label: string, group: Group, group_label: string, order: number): CategoryInfo => ({
  id,
  label,
  folder: label,
  group,
  group_label,
  order,
});

/** De zestien categorieen zoals de Rust-kant ze geeft. */
export const CATEGORIES: CategoryInfo[] = [
  row("fotos", "Foto's", "afbeeldingen", "Afbeeldingen", 0),
  row("screenshots", "Screenshots", "afbeeldingen", "Afbeeldingen", 1),
  row("gifs", "GIF's", "afbeeldingen", "Afbeeldingen", 2),
  row("design", "Design", "afbeeldingen", "Afbeeldingen", 3),
  row("videos", "Video's", "videos", "Video's", 4),
  row("schermopnames", "Schermopnames", "videos", "Video's", 5),
  row("pdfs", "PDF's", "documenten", "Documenten", 6),
  row("documenten", "Documenten", "documenten", "Documenten", 7),
  row("rekenbladen", "Rekenbladen", "documenten", "Documenten", 8),
  row("presentaties", "Presentaties", "documenten", "Documenten", 9),
  row("audio", "Audio", "overige", "Overige", 10),
  row("archieven", "Archieven", "overige", "Overige", 11),
  row("installers", "Installers", "overige", "Overige", 12),
  row("code", "Code", "overige", "Overige", 13),
  row("overig", "Overig", "overige", "Overige", 14),
  row("duplicaten", "Duplicaten", "dubbel", "Dubbel", 15),
];

let nextId = 0;

export const file = (category: Category): FileEntry => {
  const id = nextId++;
  const name = `${category}-${id}`;
  return {
    id,
    name,
    path: `C:\\Downloads\\${name}`,
    size: 1,
    modified_ms: 0,
    category,
    is_duplicate: category === "duplicaten",
    duplicate_of: null,
    placeholder: false,
  };
};
