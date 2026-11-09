/** WebKit on Mac reports MacIntel on both Intel and Apple Silicon. Unknown hosts use Ctrl. */
export function isMac(): boolean {
  return typeof navigator !== "undefined" && /^Mac/i.test(navigator.platform);
}
