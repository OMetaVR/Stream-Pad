// Director boot: renders the promo and exposes the capture contract
//   window.promo = { frames: 450, fps: 30, seek(frame): Promise<void> }
// seek(n) puts the page in exactly the state for time n / 30 s (capture seeks forward only, so discrete UI
// steps are applied in order as their time passes) and resolves after layout, image decode and two rAFs.

import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "@/styles/index.css";
import "../promo.css";

import { createRef } from "react";
import { flushSync } from "react-dom";
import { createRoot } from "react-dom/client";
import type { PromoEditorApi } from "../editor/api";
import { Director } from "./Director";
import type { Env } from "./engine";
import { FPS, FRAMES } from "./geometry";
import type { OverlaySeq } from "./Screen";
import { buildScript } from "./script";

/** Editor-clock ms that line up with promo time 0 (boot settles first: config load, daemon connect). */
const PREROLL_MS = 4000;
const OVERLAY_BASE = "/__promo/overlay/s2";

const raf = () => new Promise<void>((r) => requestAnimationFrame(() => r()));

async function loadOverlay(): Promise<OverlaySeq | null> {
  try {
    const res = await fetch(`${OVERLAY_BASE}/manifest.json`, { cache: "no-store" });
    if (!res.ok) return null;
    const m = (await res.json()) as Omit<OverlaySeq, "base">;
    if (!m.frames || !m.width || !m.anchor) return null;
    return { ...m, base: OVERLAY_BASE };
  } catch {
    return null;
  }
}

async function waitFor<T>(get: () => T | null | undefined, label: string, timeoutMs = 20000): Promise<T> {
  const t0 = performance.now();
  for (;;) {
    const v = get();
    if (v) return v;
    if (performance.now() - t0 > timeoutMs) throw new Error(`[promo] timed out: ${label}`);
    await new Promise((r) => setTimeout(r, 20));
  }
}

async function decodeAll(doc: Document) {
  await Promise.all(
    [...doc.images].map((img) =>
      img.complete
        ? img.decode().catch(() => {})
        : new Promise<void>((r) => {
            img.addEventListener("load", () => r(), { once: true });
            img.addEventListener("error", () => r(), { once: true });
          }),
    ),
  );
  await doc.fonts.ready;
}

async function boot() {
  const root = createRoot(document.getElementById("root")!);
  const iframeRef = createRef<HTMLIFrameElement>();
  const script = buildScript();
  const overlay = await loadOverlay();
  let env: Env | null = null;
  const render = (t: number) => flushSync(() => root.render(<Director t={t} env={env} script={script} overlay={overlay} iframeRef={iframeRef} />));
  render(0);

  const ed = await waitFor(() => (iframeRef.current?.contentWindow as (Window & { promoEditor?: PromoEditorApi }) | null)?.promoEditor, "editor frame");
  // Pre-roll the editor's clock so the config is loaded and the daemon shows "Running".
  for (let ms = 50; ms <= PREROLL_MS; ms += 50) await ed.advanceTo(ms);
  await ed.until(() => ed.useEditor.getState().load.kind === "ready" && ed.useDaemon.getState().status.kind === "connected", "editor ready");
  env = { ed, pts: new Map(), cams: new Map(), rects: new Map() };

  let next = 0;
  let last = -1;
  const childMs = (t: number) => PREROLL_MS + t * 1000;

  async function seek(frame: number) {
    if (frame < last) throw new Error(`promo.seek only goes forward (${frame} after ${last})`);
    last = frame;
    const t = frame / FPS;
    const e = env!;
    // Animation events (animationstart/end — Radix unmounts dialogs on animationend) are only dispatched
    // by a rendering update, so give the page one after every clock step, before anything reads the UI.
    const step = async (ms: number) => {
      await ed.advanceTo(ms);
      await raf();
      await ed.settle();
    };
    while (next < script.events.length && script.events[next]!.t <= t + 1e-9) {
      const ev = script.events[next++]!;
      await step(childMs(ev.t));
      try {
        await ev.run(e);
      } catch (err) {
        console.error(`[promo] event "${ev.label}" @${ev.t}s failed`, err);
        throw err;
      }
      // Let style/layout catch up at the event's time, so transitions it triggers start on that tick.
      await ed.settle();
      await raf();
      await ed.settle();
    }
    await step(childMs(t));
    for (const c of script.conts) if (t >= c.t0 && t <= c.t1 + 1e-9) await c.run(e, t);
    await ed.settle();
    render(t);
    await ed.settle(2);
    await decodeAll(document);
    const doc = iframeRef.current?.contentDocument;
    if (doc) await decodeAll(doc);
    await raf();
    await raf();
  }

  (window as unknown as { promo: unknown }).promo = { frames: FRAMES, fps: FPS, seek, overlay: !!overlay };
}

void boot().catch((e) => {
  console.error(e);
  (window as unknown as { promoError: string }).promoError = String(e?.stack ?? e);
});
