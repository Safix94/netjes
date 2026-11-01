/// <reference types="vite/client" />

interface Window {
  /** Alleen gezet in dev (`src/main.tsx`). */
  __NETJES_API?: import("./lib/api").Api;
}
