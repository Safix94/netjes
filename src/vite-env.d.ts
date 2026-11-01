/// <reference types="vite/client" />

interface Window {
  /** Alleen gezet in dev (`src/main.tsx`). */
  __NETJES_API?: import("./lib/api").Api;
  /** Alleen in dev: een map scannen zonder de mapkiezer, voor geautomatiseerde tests. */
  __NETJES_DEV?: { openFolder(path: string): Promise<unknown> };
}
