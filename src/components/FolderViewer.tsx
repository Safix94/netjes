import { useEffect, useRef } from "react";
import { copy } from "../copy";
import type { CategoryInfo, FileEntry } from "../lib/api";
import { drawTypeTile } from "../lib/tiles";

export interface FolderViewerProps {
  category: CategoryInfo;
  files: FileEntry[];
  /** Per bestandspad de url van de miniatuur, of niets zolang ze er niet is. */
  thumbs: Record<string, string | undefined>;
  folderPath: string;
  onClose(): void;
  onOpenFile(path: string): void;
  onReveal(path: string): void;
}

/** Getekende tegel voor een bestand zonder miniatuur, zelfde ontwerp als op het canvas. */
function TypeTile({ category, seed }: { category: string; seed: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const ctx = ref.current?.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, 128, 128);
    drawTypeTile(ctx, category, seed, 128);
  }, [category, seed]);
  return <canvas ref={ref} width={128} height={128} aria-hidden="true" />;
}

/**
 * Het paneel met de inhoud van een categoriemap: kop, aantal en een raster van miniaturen
 * met de bestandsnaam eronder. Esc en een klik naast het paneel sluiten het.
 */
export function FolderViewer({ category, files, thumbs, folderPath, onClose, onOpenFile, onReveal }: FolderViewerProps) {
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className="viewer-bg"
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="viewer" role="dialog" aria-modal="true" aria-labelledby="viewer-title">
        <div className="viewer-h">
          <div>
            <div className="mono">{category.group_label}</div>
            <h3 id="viewer-title">{category.label}</h3>
            <p>{copy.viewerMeta(files.length)}</p>
          </div>
          <div className="viewer-actions">
            <button type="button" className="mo-btn sec" onClick={() => onReveal(folderPath)}>
              {copy.reveal}
            </button>
            <button type="button" className="mo-btn sec" onClick={onClose}>
              {copy.close}
            </button>
          </div>
        </div>
        <div className="viewer-grid">
          {files.map((file) => {
            const thumb = thumbs[file.path];
            return (
              <button
                type="button"
                className="vitem"
                key={file.id}
                title={file.name}
                onClick={() => onOpenFile(file.path)}
              >
                <span className="vthumb">
                  {thumb ? <img src={thumb} alt="" /> : <TypeTile category={file.category} seed={file.id} />}
                </span>
                <span className="mono vname">{file.name}</span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
