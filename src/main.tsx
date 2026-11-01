import React from "react";
import ReactDOM from "react-dom/client";
import "@fontsource/inter-tight/400.css";
import "@fontsource/inter-tight/500.css";
import "@fontsource/inter-tight/600.css";
import "@fontsource/inter-tight/700.css";
import "@fontsource/jetbrains-mono/500.css";
import "./styles/tokens.css";
import "./styles/app.css";
import App from "./App";
import { api } from "./lib/api";

// Alleen in dev: de devtools-console kan zo elk commando proberen, bv. `await __NETJES_API.scan(...)`.
if (import.meta.env.DEV) window.__NETJES_API = api;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
