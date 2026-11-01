// De brug naar de Rust-kant. De types volgen de Rust-structs veld voor veld (snake_case, zoals
// ze geserialiseerd worden); `Option<T>` komt als `T | null`. Een mislukt commando laat de
// belofte afwijzen met een korte Nederlandse tekst (een `string`).
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { openPath as openWithDefaultApp, revealItemInDir } from "@tauri-apps/plugin-opener";

export type Category =
  | "fotos"
  | "screenshots"
  | "gifs"
  | "design"
  | "videos"
  | "schermopnames"
  | "pdfs"
  | "documenten"
  | "rekenbladen"
  | "presentaties"
  | "audio"
  | "archieven"
  | "installers"
  | "code"
  | "overig"
  | "duplicaten";

export type Group = "afbeeldingen" | "videos" | "documenten" | "overige" | "dubbel";

export interface CategoryInfo {
  id: Category;
  label: string;
  folder: string;
  group: Group;
  group_label: string;
  order: number;
}

export interface FileEntry {
  id: number;
  name: string;
  path: string;
  size: number;
  modified_ms: number;
  category: Category;
  is_duplicate: boolean;
  duplicate_of: number | null;
  /** Staat nog in de cloud (OneDrive e.a.) en is niet volledig op deze pc. */
  placeholder: boolean;
}

export interface ScanResult {
  /** De enige schrijfwijze van de map; geef precies deze tekst door aan alle andere commando's. */
  root: string;
  files: FileEntry[];
  existing_category_folders: Category[];
  skipped: number;
}

export interface PlannedMove {
  id: number;
  src: string;
  dest: string;
  category: Category;
}

export interface OrganizePlan {
  moves: PlannedMove[];
  folders_created: string[];
}

/**
 * `id` is het id van het bestand. Uitzondering: een uitkomst met `id === LOG_SAVE_FAILED_ID` hoort
 * bij geen bestand maar meldt dat het ongedaan-maken-log niet bewaard kon worden (`ok: false`,
 * `error` bevat de reden). Zoek uitkomsten dus op id en toon die ene apart.
 */
export interface MoveOutcome {
  id: number;
  ok: boolean;
  dest: string | null;
  error: string | null;
}

export interface UndoInfo {
  count: number;
  created_at_ms: number;
}

/** `errors` kan ook een uitkomst met `id === LOG_SAVE_FAILED_ID` bevatten; zie `MoveOutcome`. */
export interface UndoResult {
  restored: number;
  errors: MoveOutcome[];
}

/** Het id van de extra uitkomst "het log kon niet bewaard worden"; geen bestand heeft dit id. */
export const LOG_SAVE_FAILED_ID = 4294967295;

export const getDownloadsDir = (): Promise<string> => invoke<string>("get_downloads_dir");

export const getCategories = (): Promise<CategoryInfo[]> => invoke<CategoryInfo[]>("get_categories");

/** Weigert schijfwortels, systeemmappen en de gebruikersmap zelf, en bewaart de scan voor `beginOrganize`. */
export const scan = (root: string): Promise<ScanResult> => invoke<ScanResult>("scan", { root });

/**
 * Plant en maakt de categoriemappen aan. `root` moet de `root` van de laatste scan zijn. Een
 * tweede start op dezelfde scan wordt geweigerd ("Scan de map opnieuw."): scan eerst opnieuw.
 */
export const beginOrganize = (root: string): Promise<OrganizePlan> =>
  invoke<OrganizePlan>("begin_organize", { root });

/** Verplaatst enkel bestanden uit het plan van `beginOrganize`; een onbekend id weigert alles. */
export const moveFiles = (root: string, ids: number[]): Promise<MoveOutcome[]> =>
  invoke<MoveOutcome[]>("move_files", { root, ids });

export const checkUndo = (root: string): Promise<UndoInfo | null> =>
  invoke<UndoInfo | null>("check_undo", { root });

export const undoOrganize = (root: string): Promise<UndoResult> =>
  invoke<UndoResult>("undo_organize", { root });

/** Zet alles terug maar laat de categoriemappen staan (opnamemodus). */
export const disorganize = (root: string): Promise<UndoResult> =>
  invoke<UndoResult>("disorganize", { root });

/** De bestanden in de categoriemap onder `root`; de ids gelden alleen binnen deze lijst. */
export const listFolderFiles = (root: string, category: Category): Promise<FileEntry[]> =>
  invoke<FileEntry[]>("list_folder_files", { root, category });

/** Per pad het pad van een png in de miniatuurcache, of `null` zonder miniatuur. `px` wordt 1 tot 1024. */
export const getThumbnails = (paths: string[], px: number): Promise<Array<string | null>> =>
  invoke<Array<string | null>>("get_thumbnails", { paths, px });

export const openPath = (path: string): Promise<void> => openWithDefaultApp(path);

export const revealInExplorer = (path: string): Promise<void> => revealItemInDir(path);

/** Het gekozen pad, of `null` als de gebruiker annuleert. */
export const chooseFolder = (defaultPath: string, title: string): Promise<string | null> =>
  open({ directory: true, multiple: false, defaultPath, title });

export const confirm = (message: string, title: string): Promise<boolean> =>
  ask(message, { title });

/** De url waarmee een `<img>` een pad uit `getThumbnails` laadt. */
export const thumbUrl = (cachePath: string): string => convertFileSrc(cachePath);

export const api = {
  getDownloadsDir,
  getCategories,
  scan,
  beginOrganize,
  moveFiles,
  checkUndo,
  undoOrganize,
  disorganize,
  listFolderFiles,
  getThumbnails,
  openPath,
  revealInExplorer,
  chooseFolder,
  confirm,
  thumbUrl,
};

export type Api = typeof api;
