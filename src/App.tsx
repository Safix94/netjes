import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Controls } from "./components/Controls";
import { ErrorList, type ErrorItem } from "./components/ErrorList";
import { FolderGroups } from "./components/FolderGroups";
import { Header } from "./components/Header";
import { Stats } from "./components/Stats";
import { StatusPill, type Phase } from "./components/StatusPill";
import { copy } from "./copy";
import {
  checkUndo,
  chooseFolder,
  confirm,
  getCategories,
  getDownloadsDir,
  revealInExplorer,
  scan,
  undoOrganize,
  type Category,
  type CategoryInfo,
  type FileEntry,
} from "./lib/api";
import { errorText } from "./lib/errors";

/** Het resultaat van de laatste gelukte scan. */
interface View {
  root: string;
  files: FileEntry[];
  existing: Category[];
}

const NONE: ReadonlySet<string> = new Set();
/** Wordt gevuld door de opruimrun (branch feat/donut-canvas). */
const NO_ERRORS: ErrorItem[] = [];
const NO_FILES: FileEntry[] = [];

function folderNameOf(path: string): string {
  const parts = path.split(/[\\/]/).filter((part) => part.length > 0);
  return parts.length > 0 ? parts[parts.length - 1] : copy.defaultFolder;
}

function joinPath(root: string, folder: string): string {
  return root.endsWith("\\") || root.endsWith("/") ? root + folder : `${root}\\${folder}`;
}

function folderPhase(busy: Phase | null, view: View | null, error: string | null): Phase {
  if (busy) return busy;
  if (view === null) return error ? "error" : "scanning";
  return view.files.length === 0 ? "tidy" : "ready";
}

function captionFor(phase: Phase): string {
  if (phase === "tidy" || phase === "done") return copy.capTidy;
  return phase === "error" ? "" : copy.capMess;
}

function statusText(phase: Phase, fileCount: number, error: string | null): string {
  switch (phase) {
    case "scanning":
      return copy.statusScanning;
    case "ready":
      return copy.statusReady(fileCount);
    case "tidy":
      return copy.statusTidy;
    case "sorting":
      return copy.statusSorting;
    case "done":
      return copy.statusDone;
    case "undoing":
      return copy.statusUndoing;
    case "error":
      return error ?? "";
    case "idle":
      return "";
  }
}

export default function App() {
  const [categories, setCategories] = useState<CategoryInfo[]>([]);
  const [view, setView] = useState<View | null>(null);
  const [undoCount, setUndoCount] = useState(0);
  const [busy, setBusy] = useState<"scanning" | "undoing" | null>("scanning");
  const [error, setError] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);

  // De canvaslaag (branch feat/donut-canvas) leest hieruit het hoopvlak en de 12 vakjes per map.
  const stageRef = useRef<HTMLDivElement>(null);
  const slotCanvases = useRef(new Map<string, HTMLCanvasElement>());
  const slotRefs = useCallback((cat: string, i: number, el: HTMLCanvasElement | null) => {
    const key = `${cat}:${i}`;
    if (el) slotCanvases.current.set(key, el);
    else slotCanvases.current.delete(key);
  }, []);

  // Alleen het antwoord op de laatste scan telt; een oudere (bv. door StrictMode) valt weg.
  const scanId = useRef(0);
  const runScan = useCallback(async (path: string) => {
    const id = ++scanId.current;
    setError(null);
    setBusy("scanning");
    try {
      const result = await scan(path);
      const undo = await checkUndo(result.root).catch(() => null);
      if (id !== scanId.current) return;
      setView({ root: result.root, files: result.files, existing: result.existing_category_folders });
      setUndoCount(undo?.count ?? 0);
    } catch (e) {
      // De vorige map blijft dan staan; zonder vorige map toont de status de fout.
      if (id === scanId.current) setError(errorText(e));
    } finally {
      if (id === scanId.current) setBusy(null);
    }
  }, []);

  useEffect(() => {
    void (async () => {
      try {
        const [cats, downloads] = await Promise.all([getCategories(), getDownloadsDir()]);
        setCategories(cats);
        await runScan(downloads);
      } catch (e) {
        setError(errorText(e));
        setBusy(null);
      }
    })();
  }, [runScan]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === "r") {
        e.preventDefault(); // anders herlaadt de webview
        setRecording((on) => !on);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const files = view?.files ?? NO_FILES;
  const { counts, stats } = useMemo(() => {
    const perCategory: Record<string, number> = {};
    for (const file of files) perCategory[file.category] = (perCategory[file.category] ?? 0) + 1;
    const sum = (...ids: Category[]) => ids.reduce((n, id) => n + (perCategory[id] ?? 0), 0);
    return {
      counts: perCategory,
      stats: {
        total: files.length,
        photos: sum("fotos", "screenshots", "gifs"),
        videos: sum("videos", "schermopnames"),
        folders: 0,
        duplicates: sum("duplicaten"),
      },
    };
  }, [files]);

  // De fase van de map zelf, en die van de statusbalk: een geweigerde nieuwe map toont een fout
  // terwijl de vorige map bruikbaar blijft.
  const phase = folderPhase(busy, view, error);
  const pillPhase: Phase = busy ?? (error ? "error" : phase);

  const handleChooseFolder = async () => {
    setError(null);
    try {
      const picked = await chooseFolder(view?.root ?? "", copy.chooseFolder);
      if (picked) await runScan(picked);
    } catch (e) {
      setError(errorText(e));
    }
  };

  const handleUndo = async () => {
    if (!view) return;
    const root = view.root;
    setError(null);
    try {
      if (!(await confirm(copy.confirmUndo(undoCount), copy.appName))) return;
    } catch (e) {
      setError(errorText(e));
      return;
    }
    setBusy("undoing");
    let failure: string | null = null;
    try {
      await undoOrganize(root);
    } catch (e) {
      failure = errorText(e);
    }
    await runScan(root);
    if (failure) setError(failure);
  };

  // Een map die nog niet bestaat openen zou in Windows een foutvenster geven, dus dan gebeurt er niets.
  const handleOpen = (cat: string) => {
    setError(null);
    const info = categories.find((c) => c.id === cat);
    if (!view || !info || !view.existing.includes(info.id)) return;
    revealInExplorer(joinPath(view.root, info.folder)).catch((e) => setError(errorText(e)));
  };

  const caption = captionFor(phase);

  return (
    <div className="mo">
      <Header
        folderName={folderNameOf(view?.root ?? "")}
        recording={recording}
        onToggleRecording={() => setRecording((on) => !on)}
      />
      <StatusPill phase={pillPhase} text={statusText(pillPhase, files.length, error)} />
      <Stats {...stats} />
      <div className="mo-pile">
        <div className="stage" ref={stageRef}>
          {/* FlyCanvas komt hier, branch feat/donut-canvas */}
        </div>
        {caption && (
          <div className="mono">
            <span className="dot" />
            {caption}
          </div>
        )}
      </div>
      <Controls
        phase={phase}
        fileCount={files.length}
        recording={recording}
        canUndo={undoCount > 0}
        onOrganize={() => {
          // koppeling volgt uit branch feat/donut-canvas
        }}
        onUndo={() => void handleUndo()}
        onDisorganize={() => {}}
        onResort={() => {}}
        onChooseFolder={() => void handleChooseFolder()}
        rootPath={view?.root ?? ""}
      >
        <ErrorList errors={NO_ERRORS} />
      </Controls>
      <FolderGroups
        categories={categories}
        counts={counts}
        active={NONE}
        hit={NONE}
        slotRefs={slotRefs}
        onOpen={handleOpen}
      />
    </div>
  );
}
