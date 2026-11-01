import { copy } from "../copy";

export interface ErrorItem {
  name: string;
  error: string;
}

/** De bestanden die niet verplaatst konden worden. Toont niets zolang de lijst leeg is. */
export function ErrorList({ errors }: { errors: ErrorItem[] }) {
  if (errors.length === 0) return null;
  return (
    <div className="errors" role="alert">
      <div className="mono">{copy.errorsTitle(errors.length)}</div>
      <ul>
        {errors.map((item, i) => (
          <li key={`${item.name}-${i}`}>
            <span className="err-name">{item.name}</span> <span>{item.error}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
