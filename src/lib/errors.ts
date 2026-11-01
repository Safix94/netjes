import { copy } from "../copy";

/**
 * De zinnen die de Rust-kant zelf als fout teruggeeft en die al goed Nederlands zijn. Alles
 * anders (foutteksten van het besturingssysteem of van plugins, vaak Engels) wordt niet getoond.
 */
const KNOWN_MESSAGES: ReadonlySet<string> = new Set([
  copy.forbiddenRoot,
  copy.notAFolder,
  copy.noDownloads,
  copy.rescanFirst,
]);

/** De tekst die de gebruiker te zien krijgt bij een mislukte actie. */
export function errorText(error: unknown): string {
  return typeof error === "string" && KNOWN_MESSAGES.has(error) ? error : copy.genericError;
}
