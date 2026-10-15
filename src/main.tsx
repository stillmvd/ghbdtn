import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import "./app.css";
import { Flyout } from "./flyout/Flyout";
import { Overlay } from "./overlay/Overlay";
import { Settings } from "./settings/Settings";

const NAV = new Set(["Tab", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown"]);
const root = document.documentElement;
addEventListener("keydown", (e) => { if (NAV.has(e.key)) root.setAttribute("data-keys", ""); }, true);
addEventListener("pointerdown", () => root.removeAttribute("data-keys"), true);

const label = getCurrentWindow().label;
root.dataset.window = label;
const View = label === "flyout" ? Flyout : label === "overlay" ? Overlay : Settings;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <View />
  </React.StrictMode>,
);
