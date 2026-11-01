import { useMemo } from "react";
import type { CategoryInfo, Group } from "../lib/api";
import { FolderCard, type SlotRef } from "./FolderCard";

export interface FolderGroupsProps {
  categories: CategoryInfo[];
  counts: Record<string, number>;
  active: ReadonlySet<string>;
  hit: ReadonlySet<string>;
  slotRefs: SlotRef;
  onOpen(cat: string): void;
}

/** De rijen van de lay-out; binnen een rij staan de groepen naast elkaar. */
const ROWS: Group[][] = [["afbeeldingen", "videos"], ["documenten", "dubbel"], ["overige"]];

export function FolderGroups({ categories, counts, active, hit, slotRefs, onOpen }: FolderGroupsProps) {
  const byGroup = useMemo(() => {
    const groups = new Map<Group, CategoryInfo[]>();
    for (const info of [...categories].sort((a, b) => a.order - b.order)) {
      groups.set(info.group, [...(groups.get(info.group) ?? []), info]);
    }
    return groups;
  }, [categories]);

  return (
    <div className="cards">
      {ROWS.map((row) => (
        <div className="crow" key={row.join("-")}>
          {row.map((group) => {
            const infos = byGroup.get(group);
            if (!infos) return null;
            return (
              <section key={group}>
                <h2 className="mono grp-title">{infos[0].group_label}</h2>
                <div className="grp-grid">
                  {infos.map((info) => (
                    <FolderCard
                      key={info.id}
                      info={info}
                      count={counts[info.id] ?? 0}
                      active={active.has(info.id)}
                      hit={hit.has(info.id)}
                      wide={group === "dubbel"}
                      slotRefs={slotRefs}
                      onOpen={onOpen}
                    />
                  ))}
                </div>
              </section>
            );
          })}
        </div>
      ))}
    </div>
  );
}
