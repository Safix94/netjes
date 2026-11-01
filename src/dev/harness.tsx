// Ontwikkelharnas: de donut, de vluchten en de kaarten zonder Tauri, met nepbestanden en
// nepuitkomsten. Open via `pnpm dev` op http://localhost:1420/harness.html?n=1000
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import ReactDOM from "react-dom/client";
import { FlyCanvas, type AnimFile, type Phase } from "../components/FlyCanvas";
import type { CategoryInfo, MoveOutcome, OrganizePlan } from "../lib/api";
import { flightParams } from "../lib/flight";
import { countLabel } from "../lib/format";
import { layoutPile } from "../lib/pile";
import { makeRng } from "../lib/rng";
import { launchOrder, pickAnimated, runOrganize, type Outcome, type SortItem } from "../lib/sorter";
import { FAKE_CATEGORIES, fakeThumb, makeFakeFiles, type FakeFile } from "./fake-files";
import "./harness.css";

const params = new URLSearchParams(location.search);
const N = Math.max(0, Math.min(5000, Number(params.get("n")) || 431));
const FAILS = Math.max(0, Number(params.get("fail") ?? 4));
const SEED = Number(params.get("seed")) || 5273572;
/** `?snap=2000`: start meteen en bevries de animatie 2000 ms na de klik, voor een screenshot. */
const SNAP = Math.max(0, Number(params.get("snap")) || 0);
const ROOT = "C:\\Users\\Karsten\\Downloads";
const RADIUS = 160;
const SLOTS = 12;
const ROWS: Array<Array<CategoryInfo["group"]>> = [["afbeeldingen", "videos"], ["documenten", "dubbel"], ["overige"]];

interface Model {
  fakes: FakeFile[];
  items: SortItem[];
  anim: AnimFile[];
  failing: Set<number>;
}

/** Bouwt de hoop: eerst kiezen welke tegels vliegen, dan de phyllotaxis voor precies dat aantal. */
function buildModel(n: number, seed: number): Model {
  const fakes = makeFakeFiles(n, seed);
  const base: SortItem[] = fakes.map((f) => ({ id: f.id, category: f.category, radius: -1 }));
  const animated = pickAnimated(base);
  const animIds = fakes.filter((f) => animated.has(f.id)).map((f) => f.id);
  const pile = layoutPile(animIds.length, RADIUS, seed ^ n);
  const rnd = makeRng(seed + 1);
  const byId = new Map<number, AnimFile>();
  animIds.forEach((id, i) => {
    const fake = fakes[id];
    byId.set(id, {
      id,
      category: fake.category,
      pile: pile[i],
      flight: flightParams(rnd),
      slot: 0,
      state: "pile",
    });
  });
  const items = base.map((it) => ({ ...it, radius: byId.get(it.id)?.pile.radius ?? -1 }));
  // Vakjes in lanceervolgorde: wie eerst landt, krijgt vakje 0.
  const perCat = new Map<string, number>();
  for (const id of launchOrder(items)) {
    const file = byId.get(id);
    if (!file) continue;
    const next = perCat.get(file.category) ?? 0;
    file.slot = next;
    perCat.set(file.category, next + 1);
  }
  const failRnd = makeRng(seed + 2);
  const failing = new Set<number>();
  while (failing.size < Math.min(FAILS, animIds.length)) failing.add(animIds[Math.floor(failRnd() * animIds.length)]);
  return { fakes, items, anim: [...byId.values()], failing };
}

/** Nep-API met wachttijden zoals een echte schijf: alles lukt, behalve de gekozen ids. */
function fakeApi(failing: Set<number>, allIds: number[]) {
  const rnd = makeRng(99);
  const wait = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));
  return {
    beginOrganize: async (_root: string): Promise<OrganizePlan> => {
      await wait(80);
      return {
        moves: allIds.map((id) => ({ id, src: `${ROOT}\\${id}`, dest: `${ROOT}\\map\\${id}`, category: "fotos" })),
        folders_created: [],
      };
    },
    moveFiles: async (_root: string, ids: number[]): Promise<MoveOutcome[]> => {
      await wait(150 + rnd() * 300);
      return ids.map((id) =>
        failing.has(id)
          ? { id, ok: false, dest: null, error: "Het bestand is in gebruik door een ander programma" }
          : { id, ok: true, dest: `${ROOT}\\map\\${id}`, error: null },
      );
    },
  };
}

function Fps({ paused }: { paused: boolean }) {
  const [text, setText] = useState("fps …");
  const [slow, setSlow] = useState(false);
  useEffect(() => {
    let frames = 0;
    let worst = 0;
    let last = performance.now();
    let since = last;
    let raf = 0;
    const loop = (now: number) => {
      raf = requestAnimationFrame(loop);
      frames++;
      worst = Math.max(worst, now - last);
      last = now;
      if (now - since >= 500) {
        const fps = Math.round((frames * 1000) / (now - since));
        setText(`${fps} fps · langste frame ${worst.toFixed(1)} ms`);
        setSlow(worst > 20);
        frames = 0;
        worst = 0;
        since = now;
      }
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, []);
  if (paused) return <div className="mono fps">Pauze · de tijd staat stil</div>;
  return <div className={`mono fps${slow ? " slow" : ""}`}>{text}</div>;
}

function Harness() {
  const [model, setModel] = useState(() => buildModel(N, SEED));
  const [files, setFiles] = useState<AnimFile[]>(model.anim);
  const [phase, setPhase] = useState<Phase>("ready");
  const [counts, setCounts] = useState<Record<string, number>>({});
  const [hit, setHit] = useState<Set<string>>(new Set());
  const [errors, setErrors] = useState<Outcome[]>([]);
  const [silentOk, setSilentOk] = useState(0);
  const outcomes = useRef(new Map<number, Outcome>());
  const stageRef = useRef<HTMLDivElement>(null);
  const slots = useRef(new Map<string, HTMLDivElement>());
  const hitTimers = useRef(new Map<string, number>());
  const settled = useRef(0);
  // Pauze: elke frame wordt nowOffset zo gezet dat performance.now() + nowOffset op het bevroren tijdstip blijft.
  const [offset, setOffset] = useState(0);
  const [paused, setPaused] = useState(false);
  const frozenAt = useRef(0);
  const offsetRef = useRef(0);
  offsetRef.current = offset;
  const pause = useCallback(() => {
    frozenAt.current = performance.now() + offsetRef.current;
    setPaused(true);
  }, []);
  useEffect(() => {
    if (!paused) return;
    let raf = 0;
    const tick = () => {
      raf = requestAnimationFrame(tick);
      setOffset(frozenAt.current - performance.now());
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [paused]);
  const animatedIds = useMemo(() => new Set(model.anim.map((f) => f.id)), [model]);
  const byId = useMemo(() => new Map(model.fakes.map((f) => [f.id, f])), [model]);

  // Miniaturen komen binnen in batches van 48, zoals getThumbnails in de app.
  useEffect(() => {
    const withThumb = model.anim.filter((f) => byId.get(f.id)?.thumbSeed !== undefined).map((f) => f.id);
    let i = 0;
    let timer = 0;
    const next = () => {
      const batch = withThumb.slice(i, i + 48);
      i += 48;
      if (!batch.length) return;
      const urls = new Map(batch.map((id) => [id, fakeThumb(byId.get(id)!.thumbSeed!)]));
      setFiles((prev) => prev.map((f) => (urls.has(f.id) ? { ...f, thumb: urls.get(f.id) } : f)));
      timer = window.setTimeout(next, 60);
    };
    timer = window.setTimeout(next, 200);
    return () => clearTimeout(timer);
  }, [model, byId]);

  const getSlotRect = useCallback((cat: string, i: number) => {
    const el = slots.current.get(`${cat}:${i}`);
    return el ? el.getBoundingClientRect() : null;
  }, []);
  // Eén stabiele ref-callback per vakje, anders koppelt React bij elke render 192 refs los en weer vast.
  const slotRefs = useRef(new Map<string, (el: HTMLDivElement | null) => void>());
  const slotRef = useCallback((key: string) => {
    let cb = slotRefs.current.get(key);
    if (!cb) {
      cb = (el) => {
        if (el) slots.current.set(key, el);
        else slots.current.delete(key);
      };
      slotRefs.current.set(key, cb);
    }
    return cb;
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

  const finishIfDone = useCallback(() => {
    settled.current++;
    if (settled.current >= animatedIds.size) setPhase("done");
  }, [animatedIds]);

  const onLand = useCallback(
    (id: number) => {
      const cat = byId.get(id)?.category ?? "overig";
      setCounts((c) => ({ ...c, [cat]: (c[cat] ?? 0) + 1 }));
      flash(cat);
      finishIfDone();
    },
    [byId, flash, finishIfDone],
  );

  const onReturn = useCallback(
    (id: number) => {
      const o = outcomes.current.get(id);
      if (o) setErrors((e) => [...e, o]);
      finishIfDone();
    },
    [finishIfDone],
  );

  const reset = useCallback(() => {
    const fresh = buildModel(N, SEED);
    outcomes.current = new Map();
    settled.current = 0;
    setModel(fresh);
    setFiles(fresh.anim);
    setCounts({});
    setErrors([]);
    setSilentOk(0);
    setPaused(false);
    setOffset(0);
    setPhase("ready");
  }, []);

  const organize = useCallback(async () => {
    if (phase === "sorting") return;
    if (phase === "done") {
      reset();
      await new Promise((r) => setTimeout(r, 350));
    }
    const current = buildModelRef.current;
    outcomes.current = new Map();
    settled.current = 0;
    const api = fakeApi(current.failing, current.items.map((i) => i.id));
    const jitterOf = new Map(current.anim.map((f) => [f.id, f.flight.jitter]));
    await runOrganize({
      root: ROOT,
      files: current.items,
      api,
      animated: animatedIds,
      now: () => performance.now(),
      jitter: (id) => jitterOf.get(id) ?? 0,
      onSchedule: (launchAt) => {
        setFiles((prev) => prev.map((f) => ({ ...f, launchAt: launchAt.get(f.id) })));
        setPhase("sorting");
        if (SNAP) window.setTimeout(pause, SNAP);
      },
      onOutcome: (o) => {
        outcomes.current.set(o.id, o);
        if (!animatedIds.has(o.id)) {
          if (o.ok) {
            setSilentOk((n) => n + 1);
            const cat = byId.get(o.id)?.category ?? "overig";
            setCounts((c) => ({ ...c, [cat]: (c[cat] ?? 0) + 1 }));
          } else setErrors((e) => [...e, o]);
        }
      },
    });
  }, [phase, reset, animatedIds, byId, pause]);

  const buildModelRef = useRef(model);
  buildModelRef.current = model;
  const organizeRef = useRef(organize);
  organizeRef.current = organize;

  // ?snap=: zelf starten zodra de eerste miniaturen binnen zijn.
  useEffect(() => {
    if (!SNAP) return;
    const t = window.setTimeout(() => organizeRef.current(), 900);
    return () => clearTimeout(t);
  }, []);

  const total = Object.values(counts).reduce((a, b) => a + b, 0);
  const photos = (counts.fotos ?? 0) + (counts.screenshots ?? 0) + (counts.gifs ?? 0);
  const videos = (counts.videos ?? 0) + (counts.schermopnames ?? 0);
  const folders = Object.values(counts).filter((v) => v > 0).length;
  const statusText =
    phase === "sorting"
      ? "Bezig met sorteren"
      : phase === "done"
        ? "Klaar · klik een map om te kijken"
        : `Klaar · ${countLabel(N, "bestand", "bestanden")}`;

  return (
    <>
      <div className="app">
        <div className="head">
          <div className="mono">
            <span className="dot" />
            Netjes · ontwikkelharnas
          </div>
          <h1>
            Mijn <em>Downloads</em> opruimen
          </h1>
        </div>
        <div className="status">
          <div className={`pill ${phase}`}>
            <span className="pdot" />
            <span>{statusText}</span>
          </div>
        </div>
        <div className="stats">
          {[
            ["Bestanden", phase === "ready" ? N : total],
            ["Foto's", photos],
            ["Video's", videos],
            ["Mappen gemaakt", folders],
            ["Duplicaten", counts.duplicaten ?? 0],
          ].map(([label, value]) => (
            <div className="stat" key={label}>
              <span className="mono">{label}</span>
              <span className="stat-v">{value}</span>
            </div>
          ))}
        </div>
        <div className="pile">
          <div className="stage" ref={stageRef} />
          <div className={`mono caption${phase === "sorting" ? " dim" : ""}`}>
            <span className="dot" />
            {phase === "done" ? "Opgeruimd" : "Een rommeltje"}
          </div>
        </div>
        <div className="ctrl">
          <button className="btn" onClick={organize} disabled={phase === "sorting"}>
            {phase === "done" ? "Opnieuw sorteren" : `${countLabel(N, "bestand", "bestanden")} opruimen`}
          </button>
          <button className="btn sec" onClick={reset} disabled={phase === "sorting"}>
            Opnieuw
          </button>
          {phase === "sorting" && (
            <button className="btn sec" onClick={paused ? () => setPaused(false) : pause}>
              {paused ? "Verder" : "Pauze"}
            </button>
          )}
          <span className="mono path">{ROOT}</span>
          <Fps paused={paused} />
        </div>
        <div className="cards">
          {ROWS.map((row, r) => (
            <div className="crow" key={r}>
              {row.map((group) => {
                const cats = FAKE_CATEGORIES.filter((c) => c.group === group);
                return (
                  <div key={group}>
                    <div className="mono grp-title">{cats[0].group_label}</div>
                    <div className="grp-grid">
                      {cats.map((c) => (
                        <div
                          key={c.id}
                          className={`card${group === "dubbel" ? " wide" : ""}${hit.has(c.id) ? " hit" : ""}${(counts[c.id] ?? 0) > 0 ? " active" : ""}`}
                        >
                          <div className="card-top">
                            <span className="cdot" />
                            <span className="cname">{c.label}</span>
                            <span className="ccount">{counts[c.id] ?? 0}</span>
                          </div>
                          <div className="strip">
                            {Array.from({ length: SLOTS }, (_, i) => (
                              <div className="slot" key={i} ref={slotRef(`${c.id}:${i}`)} />
                            ))}
                          </div>
                        </div>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          ))}
        </div>
        <FlyCanvas
          files={files}
          phase={phase}
          stageRef={stageRef}
          getSlotRect={getSlotRect}
          nowOffset={offset}
          onLand={onLand}
          onReturn={onReturn}
          outcomes={outcomes.current}
        />
      </div>
      <div className="toolbar">
        <span className="mono">
          {countLabel(N, "bestand", "bestanden")} · {countLabel(model.anim.length, "tegel", "tegels")} geanimeerd ·{" "}
          {countLabel(N - model.anim.length, "bestand", "bestanden")} stil mee · {silentOk} stil verplaatst
        </span>
        <a href="?n=431">431</a>
        <a href="?n=1000">1000</a>
        <a href="?n=2000">2000</a>
        <a href="?n=66">66</a>
      </div>
      {errors.length > 0 && (
        <div className="errors">
          <span className="mono">{countLabel(errors.length, "bestand", "bestanden")} niet verplaatst</span>
          {errors.map((e) => (
            <span key={e.id}>
              {byId.get(e.id)?.name ?? e.id}: {e.error}
            </span>
          ))}
        </div>
      )}
    </>
  );
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <Harness />
  </React.StrictMode>,
);
