import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { api } from "./lib/api";

// Alleen in dev: de devtools-console kan zo elk commando proberen, bv. `await __NETJES_API.scan(...)`.
if (import.meta.env.DEV) window.__NETJES_API = api;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
