import { useMemo } from "react";
import type { CategoryInfo } from "../lib/api";

export const SLOT_COUNT = 12;

export type SlotRef = (cat: string, i: number, el: HTMLCanvasElement | null) => void;

export interface FolderCardProps {
  info: CategoryInfo;
  count: number;
  active: boolean;
  hit: boolean;
  wide: boolean;
  slotRefs: SlotRef;
  onOpen(cat: string): void;
}

export function FolderCard({ info, count, active, hit, wide, slotRefs, onOpen }: FolderCardProps) {
  // Vaste ref-functies per vakje: een nieuwe functie bij elke render zou slotRefs telkens eerst
  // met null en dan opnieuw met hetzelfde canvas aanroepen. De accolades zijn bewust: een
  // teruggegeven waarde leest React 19 als opruimfunctie van de ref.
  const refs = useMemo(
    () =>
      Array.from({ length: SLOT_COUNT }, (_, i) => (el: HTMLCanvasElement | null) => {
        slotRefs(info.id, i, el);
      }),
    [info.id, slotRefs],
  );
  const className = ["mo-card", wide && "wide", active && "active", hit && "hit"]
    .filter(Boolean)
    .join(" ");
  return (
    <button type="button" className={className} data-cat={info.id} onClick={() => onOpen(info.id)}>
      <span className="mo-card-top">
        <span className="cdot" />
        <span className="cname">{info.label}</span>
        <span className="ccount">{count}</span>
      </span>
      <span className="strip">
        {refs.map((ref, i) => (
          <span className="slot" key={i}>
            <canvas width={40} height={40} ref={ref} />
          </span>
        ))}
      </span>
    </button>
  );
}
