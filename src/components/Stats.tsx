import { copy } from "../copy";
import { RollingNumber } from "./RollingNumber";

export interface StatsProps {
  total: number;
  photos: number;
  videos: number;
  folders: number;
  duplicates: number;
}

const ORDER: Array<keyof StatsProps> = ["total", "photos", "videos", "folders", "duplicates"];

export function Stats(props: StatsProps) {
  return (
    <div className="mo-stats">
      {ORDER.map((key) => (
        <div className="mo-stat" key={key}>
          <span className="mono">{copy.stats[key]}</span>
          <span className="stat-v">
            <RollingNumber value={props[key]} />
          </span>
        </div>
      ))}
    </div>
  );
}
