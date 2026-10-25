// The director's remote control for the embedded editor. Everything here runs in the editor's realm, so
// events are built with this document's constructors and React sees them exactly like real input.

import { mockControls } from "@/lib/backend.mock";
import { getBackend } from "@/lib/backend";
import type { DaemonEvent } from "@/lib/types";
import { useDaemon } from "@/state/daemon";
import { editor, useEditor } from "@/state/store";
import { advanceTo, clockNow, macrotask, realNow, settle } from "../clock";

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

function box(el: Element | null): Box | null {
  if (!el) return null;
  const r = el.getBoundingClientRect();
  return { x: r.left, y: r.top, w: r.width, h: r.height };
}

/** First element matching `selector` whose text includes `text` (when given). */
function find(selector: string, text?: string): HTMLElement | null {
  const all = [...document.querySelectorAll<HTMLElement>(selector)];
  return (text ? all.find((e) => (e.textContent ?? "").includes(text)) : all[0]) ?? null;
}

function pointerInit(x: number, y: number, buttons: number): PointerEventInit {
  return { bubbles: true, cancelable: true, composed: true, clientX: x, clientY: y, screenX: x, screenY: y, button: 0, buttons, pointerId: 1, pointerType: "mouse", isPrimary: true, view: window };
}

let runId = 40;

// The app's own CSS animations and transitions (dialog/popover entrances, selection rings, the trace ping)
// stay on, but are held (rate 0) and stepped on the virtual clock — so they look exactly as shipped and
// land on the same frame in every capture. Rate 0 rather than pause(): a paused animation may never send
// animationstart (Radix Presence relies on it), and whether it does would depend on rendering order.
const startedAt = new WeakMap<Animation, number>();
function pinAnimations() {
  const now = clockNow();
  for (const a of document.getAnimations()) {
    let t0 = startedAt.get(a);
    if (t0 === undefined) {
      t0 = now;
      startedAt.set(a, t0);
      a.playbackRate = 0;
    }
    a.currentTime = now - t0;
  }
}

/** Jump the animations of elements matching `selector` to their end (e.g. skip a popover's exit). */
function finishAnimations(selector: string) {
  for (const a of document.getAnimations()) {
    const target = (a.effect as KeyframeEffect | null)?.target;
    if (target?.closest(selector)) startedAt.set(a, -1e9);
  }
  pinAnimations();
}
/** Element that received the last pointerdown — gets the moves/up, as pointer capture would. */
let captured: Element | null = null;

export function createApi() {
  const api = {
    editor,
    useEditor,
    useDaemon,
    backend: getBackend(),
    now: clockNow,
    advanceTo,
    settle,
    macrotask,
    pinAnimations,
    finishAnimations,
    nextRunId: () => ++runId,
    currentRunId: () => runId,
    emit(event: DaemonEvent) {
      mockControls()?.emit(event);
    },
    find,
    rect: (selector: string, text?: string) => box(find(selector, text)),
    /** Bounding box of every flow node. */
    nodesBox(): Box {
      const rs = [...document.querySelectorAll(".react-flow__node")].map((n) => n.getBoundingClientRect());
      const x = Math.min(...rs.map((r) => r.left));
      const y = Math.min(...rs.map((r) => r.top));
      return { x, y, w: Math.max(...rs.map((r) => r.right)) - x, h: Math.max(...rs.map((r) => r.bottom)) - y };
    },
    box,

    /** A full mouse click at a point, delivered to whatever is under it (like a real click). */
    click(x: number, y: number) {
      const el = document.elementFromPoint(x, y);
      if (!el) return null;
      el.dispatchEvent(new PointerEvent("pointerdown", pointerInit(x, y, 1)));
      el.dispatchEvent(new MouseEvent("mousedown", pointerInit(x, y, 1)));
      if (el instanceof HTMLElement) {
        const focusable = el.closest<HTMLElement>("button, input, textarea, [tabindex], a[href]");
        focusable?.focus({ preventScroll: true });
      }
      el.dispatchEvent(new PointerEvent("pointerup", pointerInit(x, y, 0)));
      el.dispatchEvent(new MouseEvent("mouseup", pointerInit(x, y, 0)));
      el.dispatchEvent(new MouseEvent("click", pointerInit(x, y, 0)));
      pinAnimations();
      return el.tagName;
    },
    pointerDown(x: number, y: number) {
      captured = document.elementFromPoint(x, y);
      captured?.dispatchEvent(new PointerEvent("pointerdown", pointerInit(x, y, 1)));
      captured?.dispatchEvent(new MouseEvent("mousedown", pointerInit(x, y, 1)));
    },
    pointerMove(x: number, y: number) {
      (captured ?? document.elementFromPoint(x, y))?.dispatchEvent(new PointerEvent("pointermove", pointerInit(x, y, captured ? 1 : 0)));
    },
    pointerUp(x: number, y: number) {
      const el = captured ?? document.elementFromPoint(x, y);
      el?.dispatchEvent(new PointerEvent("pointerup", pointerInit(x, y, 0)));
      el?.dispatchEvent(new MouseEvent("mouseup", pointerInit(x, y, 0)));
      captured = null;
    },

    /** keydown (+ keyup) on the focused element — reaches React handlers and the global hotkey table. */
    key(key: string) {
      const target = document.activeElement ?? document.body;
      const init: KeyboardEventInit = { key, code: key === "Enter" ? "Enter" : undefined, bubbles: true, cancelable: true, composed: true };
      target.dispatchEvent(new KeyboardEvent("keydown", init));
      target.dispatchEvent(new KeyboardEvent("keyup", init));
      pinAnimations();
    },
    /** Type into the focused input/textarea, one character, like a keyboard would. */
    type(ch: string) {
      const el = document.activeElement;
      if (!(el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement)) return false;
      el.dispatchEvent(new KeyboardEvent("keydown", { key: ch, bubbles: true, cancelable: true }));
      const proto = el instanceof HTMLInputElement ? HTMLInputElement.prototype : HTMLTextAreaElement.prototype;
      Object.getOwnPropertyDescriptor(proto, "value")!.set!.call(el, el.value + ch);
      el.dispatchEvent(new InputEvent("input", { bubbles: true, data: ch, inputType: "insertText" }));
      el.dispatchEvent(new KeyboardEvent("keyup", { key: ch, bubbles: true }));
      return true;
    },

    /** HTML5 file drag over / drop at a point (the numpad's cover drop target). */
    async fileDrag(phase: "over" | "drop" | "leave", x: number, y: number, url: string, name: string) {
      const el = document.elementFromPoint(x, y);
      if (!el) return;
      const dt = new DataTransfer();
      const blob = await (await fetch(url)).blob();
      dt.items.add(new File([blob], name, { type: blob.type || "image/png" }));
      const init: DragEventInit = { bubbles: true, cancelable: true, composed: true, clientX: x, clientY: y, dataTransfer: dt };
      if (phase === "over") {
        el.dispatchEvent(new DragEvent("dragenter", init));
        el.dispatchEvent(new DragEvent("dragover", init));
      } else if (phase === "leave") {
        el.dispatchEvent(new DragEvent("dragleave", init));
      } else {
        el.dispatchEvent(new DragEvent("dragover", init));
        el.dispatchEvent(new DragEvent("drop", init));
      }
      // Style changes from the synchronous part of the handlers start their transitions now, not whenever
      // the next rendering happens to run (the async part — importing the file — commits later).
      pinAnimations();
    },

    /** Resolve when `cond` holds (polled on real time — for real async work like image decoding). */
    async until(cond: () => unknown, label: string, timeoutMs = 8000) {
      const t0 = realNow();
      for (let i = 0; !cond(); i++) {
        if (realNow() - t0 > timeoutMs) throw new Error(`[promo] timed out waiting for ${label}`);
        await macrotask();
        if (i % 20 === 19) await settle(1);
      }
    },

    /** Every image in the document has loaded and decoded. */
    async images() {
      const imgs = [...document.images];
      await Promise.all(imgs.map((img) => (img.complete ? img.decode().catch(() => {}) : new Promise<void>((r) => {
              img.addEventListener("load", () => r(), { once: true });
              img.addEventListener("error", () => r(), { once: true });
            }))));
      await document.fonts.ready;
    },
  };
  return api;
}

export type PromoEditorApi = ReturnType<typeof createApi>;
