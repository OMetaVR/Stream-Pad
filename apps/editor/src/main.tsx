import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "./styles/index.css";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { initBackend, isTauri } from "./lib/backend";
import { initTheme } from "./lib/theme";
import { flushSave, startAutosave } from "./state/autosave";
import { startDaemonStore } from "./state/daemon";
import { editor } from "./state/store";

const FLUSH_TIMEOUT_MS = 2500;

/** Write pending edits before the window goes away. */
async function installCloseHandler() {
  if (isTauri()) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    // With a listener registered Tauri waits for it, then destroys the window.
    await getCurrentWindow().onCloseRequested(async () => {
      await Promise.race([flushSave(), new Promise((r) => setTimeout(r, FLUSH_TIMEOUT_MS))]);
    });
  } else {
    window.addEventListener("pagehide", () => void flushSave());
  }
}

function renderFatal(root: HTMLElement, error: unknown) {
  const message = error instanceof Error ? error.message : String(error);
  root.innerHTML = "";
  const box = document.createElement("pre");
  box.style.cssText = "margin:48px;color:#f0626b;font:13px/20px monospace;white-space:pre-wrap";
  box.textContent = `StreamPad could not start:\n${message}`;
  root.append(box);
}

async function boot() {
  const root = document.getElementById("root")!;
  initTheme();
  try {
    await initBackend();
  } catch (e) {
    renderFatal(root, e);
    return;
  }
  startDaemonStore();
  startAutosave();
  void editor.load();
  void installCloseHandler().catch((e) => console.error("close handler", e));
  // Dev-only UI kit reference at #kit (tree-shaken from production builds).
  const Root = import.meta.env.DEV && location.hash === "#kit" ? (await import("./dev/KitGallery")).KitGallery : App;
  createRoot(root).render(
    <StrictMode>
      <Root />
    </StrictMode>,
  );
}

void boot();
