import type { ReactNode } from "react";
import { copy } from "../copy";
import type { Phase } from "./StatusPill";

export interface ControlsProps {
  phase: Phase;
  fileCount: number;
  recording: boolean;
  canUndo: boolean;
  onOrganize(): void;
  onUndo(): void;
  onDisorganize(): void;
  onResort(): void;
  onChooseFolder(): void;
  rootPath: string;
  /** Staat onder de padregel, bv. de lijst met bestanden die niet verplaatst konden worden. */
  children?: ReactNode;
}

/** Fases waarin Opruimen niets te doen heeft of niet mag starten. */
const NO_ORGANIZE: ReadonlySet<Phase> = new Set<Phase>(["scanning", "tidy", "sorting", "undoing", "error"]);
/** Fases waarin de map in beweging is en de andere knoppen even wachten. */
const BUSY: ReadonlySet<Phase> = new Set<Phase>(["scanning", "sorting", "undoing"]);

export function Controls({
  phase,
  fileCount,
  recording,
  canUndo,
  onOrganize,
  onUndo,
  onDisorganize,
  onResort,
  onChooseFolder,
  rootPath,
  children,
}: ControlsProps) {
  const busy = BUSY.has(phase);
  return (
    <div className="mo-ctrl">
      <button type="button" className="mo-btn" disabled={NO_ORGANIZE.has(phase)} onClick={onOrganize}>
        {copy.organize(fileCount)}
      </button>
      {recording && (
        <>
          <button type="button" className="mo-btn sec" disabled={busy} onClick={onResort}>
            {copy.resort}
          </button>
          <button type="button" className="mo-btn sec" disabled={busy} onClick={onDisorganize}>
            {copy.disorganize}
          </button>
        </>
      )}
      <button type="button" className="mo-btn sec" disabled={busy || !canUndo} onClick={onUndo}>
        {copy.undo}
      </button>
      <button type="button" className="mo-btn sec" disabled={busy} onClick={onChooseFolder}>
        {copy.chooseFolder}
      </button>
      <span className="mono path" title={rootPath}>
        <bdi>{rootPath}</bdi>
      </span>
      {children}
    </div>
  );
}
