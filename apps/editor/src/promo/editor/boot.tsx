// The real editor, booted for the promo inside the director's iframe: same App, styles, stores and mock
// backend as `bun run dev`, but on the virtual clock (../clock), with fresh sample data, no Simulate panel
// and no CSS/motion animations. The director drives it through `window.promoEditor` (./api.ts).

import { onTick, settle } from "../clock"; // first: installs the virtual clock before any app module loads
import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "@/styles/index.css";
import "../promo.css";

import { MotionGlobalConfig } from "motion/react";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "@/App";
import { getBackend, initBackend } from "@/lib/backend";
import { initTheme } from "@/lib/theme";
import { startAutosave } from "@/state/autosave";
import { startDaemonStore } from "@/state/daemon";
import { editor } from "@/state/store";
import { createApi } from "./api";

MotionGlobalConfig.skipAnimations = true;

// Fresh, deterministic state on every load (the mock persists config/covers/recents in localStorage).
for (const k of Object.keys(localStorage)) if (k.startsWith("streampad.")) localStorage.removeItem(k);
localStorage.setItem("streampad.theme", "dark");

// Synthetic pointer events have no active pointer to capture; the crop dialog only needs the events.
const capture = Element.prototype.setPointerCapture;
Element.prototype.setPointerCapture = function (id: number) {
  try {
    capture.call(this, id);
  } catch {
    // no active pointer — fine for synthetic input
  }
};

// Clicks here are synthetic, so Chrome would treat every focus as keyboard focus and draw focus rings that a
// mouse user never sees. Focus like a mouse click does (explicit callers can still ask for the ring).
const focus = HTMLElement.prototype.focus;
HTMLElement.prototype.focus = function (options?: FocusOptions) {
  focus.call(this, { focusVisible: false, ...options } as FocusOptions);
};



/** First frame of an animated image as a PNG data URL (drawImage always paints an animation's first frame). */
async function stillOf(url: string): Promise<string> {
  const img = new Image();
  img.src = url;
  await img.decode();
  const c = document.createElement("canvas");
  c.width = img.naturalWidth;
  c.height = img.naturalHeight;
  c.getContext("2d")!.drawImage(img, 0, 0);
  return c.toDataURL("image/png");
}

/** Captured pixels must not depend on wall-clock GIF playback: serve animated covers as their first frame. */
async function freezeGifs() {
  const backend = getBackend();
  const assetUrl = backend.assetUrl.bind(backend);
  const stills = new Map<string, string>();
  for (const url of ["/mock/covers/live.gif"]) stills.set(url, await stillOf(url));
  backend.assetUrl = (ref) => {
    const url = assetUrl(ref);
    return url && /\.gif$/i.test(url) ? (stills.get(url) ?? url) : url;
  };
}

async function boot() {
  document.documentElement.style.background = "var(--sp-bg)";
  initTheme();
  await initBackend();
  await freezeGifs();
  startDaemonStore();
  startAutosave();
  void editor.load();
  const api = createApi();
  onTick(api.pinAnimations);
  // "Test" runs are driven by the director on the timeline (trace/run/log events via api.emit).
  getBackend().testMacro = async () => api.nextRunId();
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
  await settle();
  (window as unknown as { promoEditor: typeof api }).promoEditor = api;
}

void boot();
