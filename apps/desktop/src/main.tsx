import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./styles/app.css";
import { isTauriRuntime } from "./lib/tauri";

// In a plain browser there is no Mica behind the window; fake a desktop.
if (!isTauriRuntime()) document.documentElement.classList.add("browser-preview");

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
