import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Controls } from "./components/Controls";
import { ErrorList, type ErrorItem } from "./components/ErrorList";
import { FlyCanvas, type AnimFile } from "./components/FlyCanvas";
import { FolderGroups } from "./components/FolderGroups";
import { FolderViewer } from "./components/FolderViewer";
import { Header, type UpdateState } from "./components/Header";
import { Stats } from "./components/Stats";
import { StatusPill, type Phase } from "./components/StatusPill";
import { copy } from "./copy";
import {
  DOWNLOAD_PAGE,
  beginOrganize,
  checkUndo,
  checkUpdate,
  chooseFolder,
  confirm,
  disorganize,
  getCategories,
  getDownloadsDir,
  getThumbnails,
  listFolderFiles,
  moveFiles,
  openPath,
  openUrl,
  revealInExplorer,
  scan,
  thumbUrl,
  undoOrganize,
  type Category,
  type CategoryInfo,
  type FileEntry,
} from "./lib/api";
import { buildAnimModel, chunk, thumbOrder, type AnimModel } from "./lib/animModel";
import { errorText } from "./lib/errors";
import { runOrganize, type Outcome } from "./lib/sorter";

/** Het resultaat van de laatste gelukte scan, met de hoop die erbij hoort. */
interface View {
  root: string;
  files: FileEntry[];
  existing: Category[];
  model: AnimModel;
}

/** De stand van de lopende of net afgelopen opruimbeurt. */
interface Run {
  phase: "sorting" | "done";
  counts: Record<string, number>;
  errors: ErrorItem[];
  touched: ReadonlySet<string>;
  logError: boolean;
}

interface Viewer {
  info: CategoryInfo;
  files: FileEntry[];
  thumbs: Record<string, string | undefined>;
}

const NONE: ReadonlySet<string> = new Set();
const NO_FILES: FileEntry[] = [];
const NO_ERRORS: ErrorItem[] = [];
/** Miniaturen per aanvraag aan de Windows-shell. */
const THUMB_BATCH = 48;
/** Pixelmaat van de miniaturen; op een scherm met schaal 2 is het beeldvlak van een tegel 120 px. */
const THUMB_PX = 128;

function folderNameOf(path: string): string {
  const parts = path.split(/[\\/]/).filter((part) => part.length > 0);
  return parts.length > 0 ? parts[parts.length - 1] : copy.defaultFolder;
}

function joinPath(root: string, folder: string): string {
  return root.endsWith("\\") || root.endsWith("/") ? root + folder : `${root}\\${folder}`;
}

/** Dezelfde map geeft dezelfde hoop. */
function seedFor(root: string): number {
  let h = 2166136261;
  for (let i = 0; i < root.length; i++) h = Math.imul(h ^ root.charCodeAt(i), 16777619);
  return h >>> 0;
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

function sumOf(counts: Record<string, number>, ...ids: Category[]): number {
  return ids.reduce((n, id) => n + (counts[id] ?? 0), 0);
}

export default function App() {
  const [categories, setCategories] = useState<CategoryInfo[]>([]);
  const [view, setView] = useState<View | null>(null);
  const [anim, setAnim] = useState<AnimFile[]>([]);
  const [run, setRun] = useState<Run | null>(null);
  const [hit, setHit] = useState<ReadonlySet<string>>(NONE);
  const [viewer, setViewer] = useState<Viewer | null>(null);
  const [undoCount, setUndoCount] = useState(0);
  const [busy, setBusy] = useState<"scanning" | "undoing" | null>("scanning");
  const [error, setError] = useState<string | null>(null);
  const [recording, setRecording] = useState(false);
  const [update, setUpdate] = useState<{ state: UpdateState; version?: string }>({ state: "idle" });

  const handleCheckUpdate = async () => {
    setUpdate({ state: "checking" });
    try {
      const info = await checkUpdate();
      setUpdate({ state: info.state, version: info.version ?? undefined });
    } catch {
      setUpdate({ state: "error" });
    }
  };

  // De canvaslaag leest hieruit het hoopvlak en de 12 vakjes per map.
  const stageRef = useRef<HTMLDivElement>(null);
  const slotCanvases = useRef(new Map<string, HTMLCanvasElement>());
  const slotRefs = useCallback((cat: string, i: number, el: HTMLCanvasElement | null) => {
    const key = `${cat}:${i}`;
    if (el) slotCanvases.current.set(key, el);
    else slotCanvases.current.delete(key);
  }, []);
  const getSlotRect = useCallback((cat: string, i: number) => {
    const el = slotCanvases.current.get(`${cat}:${i}`);
    return el ? el.getBoundingClientRect() : null;
  }, []);

  // De lopende opruimbeurt, buiten React-state omdat het canvas er elk frame in kijkt.
  const outcomes = useRef(new Map<number, Outcome>());
  const live = useRef({
    counts: {} as Record<string, number>,
    errors: [] as ErrorItem[],
    touched: new Set<string>(),
    logError: false,
    settled: 0,
    view: null as View | null,
  });
  const hitTimers = useRef(new Map<string, number>());

  const publish = useCallback((phase: Run["phase"]) => {
    const l = live.current;
    setRun({ phase, counts: { ...l.counts }, errors: [...l.errors], touched: new Set(l.touched), logError: l.logError });
  }, []);

  // Alleen het antwoord op de laatste scan telt; een oudere (bv. door StrictMode) valt weg.
  const scanId = useRef(0);
  const runScan = useCallback(async (path: string): Promise<View | null> => {
    const id = ++scanId.current;
    setError(null);
    setBusy("scanning");
    setViewer(null);
    try {
      const result = await scan(path);
      const undo = await checkUndo(result.root).catch(() => null);
      if (id !== scanId.current) return null;
      const stage = stageRef.current?.clientWidth || 340;
      const model = buildAnimModel(result.files, Math.max(60, stage / 2 - 10), seedFor(result.root));
      const next: View = { root: result.root, files: result.files, existing: result.existing_category_folders, model };
      outcomes.current = new Map();
      live.current = { counts: {}, errors: [], touched: new Set(), logError: false, settled: 0, view: next };
      setRun(null);
      setHit(NONE);
      setView(next);
      setAnim(model.anim);
      setUndoCount(undo?.count ?? 0);
      return next;
    } catch (e) {
      // De vorige map blijft dan staan; zonder vorige map toont de status de fout.
      if (id === scanId.current) setError(errorText(e));
      return null;
    } finally {
      if (id === scanId.current) setBusy(null);
    }
  }, []);

  // Alleen in dev: een map scannen zonder de mapkiezer, voor de geautomatiseerde test.
  useEffect(() => {
    if (import.meta.env.DEV) window.__NETJES_DEV = { openFolder: runScan };
  }, [runScan]);

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

  // Miniaturen in batches van 48, buitenste ring eerst; een nieuwe scan breekt het laden af.
  useEffect(() => {
    if (!view) return;
    let cancelled = false;
    const byId = new Map(view.files.map((f) => [f.id, f]));
    void (async () => {
      for (const ids of chunk(thumbOrder(view.model.anim), THUMB_BATCH)) {
        let paths: Array<string | null>;
        try {
          paths = await getThumbnails(
            ids.map((id) => byId.get(id)!.path),
            THUMB_PX,
          );
        } catch {
          return;
        }
        if (cancelled) return;
        const found = new Map<number, string>();
        ids.forEach((id, i) => {
          const p = paths[i];
          if (p) found.set(id, p);
        });
        if (found.size > 0) setAnim((prev) => prev.map((a) => (found.has(a.id) ? { ...a, thumb: found.get(a.id) } : a)));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [view]);

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

  const flash = useCallback((cat: string) => {
    setHit((prev) => new Set(prev).add(cat));
    clearTimeout(hitTimers.current.get(cat));
    hitTimers.current.set(
      cat,
      window.setTimeout(() => {
        setHit((prev) => {
          const next = new Set(prev);
          next.delete(cat);
          return next;
        });
      }, 160),
    );
  }, []);

  const finish = useCallback(() => {
    const v = live.current.view;
    publish("done");
    if (!v) return;
    checkUndo(v.root)
      .then((undo) => setUndoCount(undo?.count ?? 0))
      .catch(() => undefined);
  }, [publish]);

  const countMove = useCallback(
    (fileId: number, o: Outcome | undefined, animated: boolean) => {
      const v = live.current.view;
      const file = v?.files.find((f) => f.id === fileId);
      if (!file) return;
      if (o?.ok) {
        live.current.counts[file.category] = (live.current.counts[file.category] ?? 0) + 1;
        live.current.touched.add(file.category);
        if (animated) flash(file.category);
      } else {
        live.current.errors.push({ name: file.name, error: o?.error ?? copy.moveFailed });
      }
      if (animated) live.current.settled++;
      const all = v ? v.model.animated.size : 0;
      if (animated && live.current.settled >= all) finish();
      else publish("sorting");
    },
    [finish, flash, publish],
  );

  const onLand = useCallback((id: number) => countMove(id, outcomes.current.get(id), true), [countMove]);
  const onReturn = useCallback((id: number) => countMove(id, outcomes.current.get(id), true), [countMove]);

  const startOrganize = useCallback(
    async (v: View) => {
      if (v.files.length === 0) return;
      setError(null);
      setViewer(null);
      outcomes.current = new Map();
      live.current = { counts: {}, errors: [], touched: new Set(), logError: false, settled: 0, view: v };
      setAnim((prev) => prev.map((a) => ({ ...a, launchAt: undefined, state: "pile" })));
      publish("sorting");
      const jitter = new Map(v.model.anim.map((a) => [a.id, a.flight.jitter]));
      try {
        const result = await runOrganize({
          root: v.root,
          files: v.model.items,
          animated: v.model.animated,
          api: { beginOrganize, moveFiles },
          now: () => performance.now(),
          jitter: (id) => jitter.get(id) ?? 0,
          onSchedule: (launchAt) => {
            setAnim((prev) => prev.map((a) => ({ ...a, launchAt: launchAt.get(a.id) })));
          },
          onOutcome: (o) => {
            outcomes.current.set(o.id, o);
            if (!v.model.animated.has(o.id)) countMove(o.id, o, false);
          },
        });
        if (result.logError) {
          live.current.logError = true;
          live.current.errors.push({ name: copy.undo, error: copy.logNotSaved });
        }
        if (v.model.animated.size === 0) finish();
        else publish(live.current.settled >= v.model.animated.size ? "done" : "sorting");
      } catch (e) {
        setRun(null);
        setError(errorText(e));
      }
    },
    [countMove, finish, publish],
  );

  const files = view?.files ?? NO_FILES;
  const scanCounts = useMemo(() => {
    const perCategory: Record<string, number> = {};
    for (const file of files) perCategory[file.category] = (perCategory[file.category] ?? 0) + 1;
    return perCategory;
  }, [files]);

  const counts = run ? run.counts : scanCounts;
  const stats = useMemo(
    () => ({
      total: run ? sumOf(run.counts, ...(Object.keys(run.counts) as Category[])) : files.length,
      photos: sumOf(counts, "fotos", "screenshots", "gifs"),
      videos: sumOf(counts, "videos", "schermopnames"),
      folders: run ? run.touched.size : 0,
      duplicates: sumOf(counts, "duplicaten"),
    }),
    [run, counts, files.length],
  );

  // De fase van de map zelf, en die van de statusbalk: een geweigerde nieuwe map toont een fout
  // terwijl de vorige map bruikbaar blijft.
  const phase: Phase = run?.phase ?? folderPhase(busy, view, error);
  const pillPhase: Phase = busy ?? (error ? "error" : phase);

  const handleOrganize = () => {
    if (!view) return;
    if (phase === "done") {
      // De bestanden zijn al verhuisd: een verse scan toont wat er nog los ligt.
      void runScan(view.root);
      return;
    }
    if (phase === "ready") void startOrganize(view);
  };

  const handleChooseFolder = async () => {
    setError(null);
    try {
      const picked = await chooseFolder(view?.root ?? "", copy.chooseFolder);
      if (picked) await runScan(picked);
    } catch (e) {
      setError(errorText(e));
    }
  };

  const restore = async (root: string, action: (root: string) => Promise<unknown>): Promise<View | null> => {
    setBusy("undoing");
    let failure: string | null = null;
    try {
      await action(root);
    } catch (e) {
      failure = errorText(e);
    }
    const next = await runScan(root);
    if (failure) setError(failure);
    return failure ? null : next;
  };

  const handleUndo = async () => {
    if (!view) return;
    setError(null);
    try {
      if (!(await confirm(copy.confirmUndo(undoCount), copy.appName))) return;
    } catch (e) {
      setError(errorText(e));
      return;
    }
    await restore(view.root, undoOrganize);
  };

  const handleDisorganize = async () => {
    if (!view) return;
    setError(null);
    try {
      if (!(await confirm(copy.confirmDisorganize, copy.appName))) return;
    } catch (e) {
      setError(errorText(e));
      return;
    }
    await restore(view.root, disorganize);
  };

  const handleResort = async () => {
    if (!view) return;
    setError(null);
    const fresh = await restore(view.root, disorganize);
    if (fresh) await startOrganize(fresh);
  };

  const folderExists = (id: Category) => !!view && (view.existing.includes(id) || live.current.touched.has(id));

  // Een map die nog niet bestaat openen zou in Windows een foutvenster geven, dus dan gebeurt er niets.
  const handleOpen = async (cat: string) => {
    setError(null);
    const info = categories.find((c) => c.id === cat);
    if (!view || !info || !folderExists(info.id)) return;
    const root = view.root;
    try {
      const list = await listFolderFiles(root, info.id);
      setViewer({ info, files: list, thumbs: {} });
      for (const batch of chunk(list, THUMB_BATCH)) {
        const paths = await getThumbnails(
          batch.map((f) => f.path),
          THUMB_PX,
        );
        setViewer((prev) => {
          if (!prev || prev.info.id !== info.id) return prev;
          const thumbs = { ...prev.thumbs };
          batch.forEach((f, i) => {
            const p = paths[i];
            if (p) thumbs[f.path] = thumbUrl(p);
          });
          return { ...prev, thumbs };
        });
      }
    } catch (e) {
      setError(errorText(e));
    }
  };

  const closeViewer = useCallback(() => setViewer(null), []);

  const caption = captionFor(phase);

  return (
    <div className="mo">
      <Header
        folderName={folderNameOf(view?.root ?? "")}
        recording={recording}
        updateState={update.state}
        updateVersion={update.version}
        onToggleRecording={() => setRecording((on) => !on)}
        onCheckUpdate={() => void handleCheckUpdate()}
        onOpenUpdate={() => openUrl(DOWNLOAD_PAGE).catch((e) => setError(errorText(e)))}
      />
      <StatusPill phase={pillPhase} text={statusText(pillPhase, files.length, error)} />
      <Stats {...stats} />
      <div className="mo-pile">
        <div className="stage" ref={stageRef} />
        {caption && (
          <div className={`mono caption${phase === "sorting" ? " dim" : ""}`}>
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
        onOrganize={handleOrganize}
        onUndo={() => void handleUndo()}
        onDisorganize={() => void handleDisorganize()}
        onResort={() => void handleResort()}
        onChooseFolder={() => void handleChooseFolder()}
        rootPath={view?.root ?? ""}
      >
        <ErrorList errors={run?.errors ?? NO_ERRORS} />
      </Controls>
      <FolderGroups
        categories={categories}
        counts={counts}
        active={run?.touched ?? NONE}
        hit={hit}
        slotRefs={slotRefs}
        onOpen={(cat) => void handleOpen(cat)}
      />
      <FlyCanvas
        files={anim}
        phase={phase}
        stageRef={stageRef}
        getSlotRect={getSlotRect}
        onLand={onLand}
        onReturn={onReturn}
        outcomes={outcomes.current}
      />
      {viewer && view && (
        <FolderViewer
          category={viewer.info}
          files={viewer.files}
          thumbs={viewer.thumbs}
          folderPath={joinPath(view.root, viewer.info.folder)}
          onClose={closeViewer}
          onOpenFile={(path) => openPath(path).catch((e) => setError(errorText(e)))}
          onReveal={(path) => revealInExplorer(path).catch((e) => setError(errorText(e)))}
        />
      )}
    </div>
  );
}
