// The promo's beats (seconds). S3/S4 drive the real editor: clicks land on whatever the real UI has under
// the cursor, typing goes into the real inputs, the crop dialog gets real pointer events, and "Test"
// traces are emitted into the daemon store on the timeline.

import { actionJs } from "@/lib/mock/compile";
import type { GraphNode } from "@/lib/types";
import { lerp, type Pt } from "../anim";
import { Script, type Env } from "./engine";
import { APP_W, frameApp, IDENTITY, TITLE_H, WIN_SCALE, WIN_X, WIN_Y, type Rect } from "./geometry";

export const AUDIO = "p_a0d10001";

export const T = {
  // S1
  fadeUp: [0, 0.3],
  caption1: 0.45,
  scrollLock: 1.05,
  wave: [1.1, 1.75],
  caption2: 1.15,
  flatten: [2.0, 2.4],
  // S2 (overlay sequence frame 0 = 2.40 s)
  s2: 2.4,
  s2End: 6.6,
  // S3
  dim: [6.1, 6.55],
  rise: [6.28, 6.8],
  // S4
  pushIn: [11.62, 11.86],
  flowSwap: 11.86,
  // S5
  outro: 13.75,
  end: 15,
} as const;

/** Key presses shown by the key-cast HUD (S2 = the physical pad, S3 = the keyboard in the editor). */
export const KEYCAST: { t: number; label: string }[] = [
  { t: 1.05, label: "Scroll Lock" },
  { t: 2.7, label: "Enter" },
  { t: 3.7, label: "5" },
  { t: 4.5, label: "+" },
  { t: 5.3, label: "1" },
  { t: 6.1, label: "Enter" },
  { t: 7.28, label: "Enter" },
  { t: 7.82, label: "Enter" },
];

export type CaptionPos = "tl" | "bc";
export const CAPTIONS: { t0: number; t1: number; text: string; pos: CaptionPos }[] = [
  { t0: 2.55, t1: 4.38, text: "Press {Enter} — it slides in.", pos: "tl" },
  { t0: 4.48, t1: 6.25, text: "{+ / −} switch profiles.", pos: "tl" },
  { t0: 11.1, t1: 12.36, text: "Design {every key.}", pos: "bc" },
  { t0: 12.4, t1: 13.74, text: "Branch, loop — or write {JavaScript.}", pos: "bc" },
];

/** Where the cursor rests before its first move (app coords). */
export const CURSOR_START: Pt = { x: 760, y: 700 };
export const CURSOR_SHOW: [number, number, number, number] = [6.6, 6.76, 13.62, 13.78];

/** Desktop file the user drags onto key 8 (screen coords of its centre). */
export const FILE = { x: 57, y: 560, url: "/mock/covers/synthwave.png", name: "synthwave.png" };

export const DRAG = { press: 9.24, t0: 9.28, t1: 9.68, drop: 9.72 };
export const TEST = { click: 12.36, step: 0.2 };

const unite = (a: Rect, b: Rect): Rect => {
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return { x, y, w: Math.max(a.x + a.w, b.x + b.w) - x, h: Math.max(a.y + a.h, b.y + b.h) - y };
};

function need(env: Env, sel: string, text?: string): Rect {
  const r = env.ed.rect(sel, text);
  if (!r) throw new Error(`[promo] missing ${sel} ${text ?? ""}`);
  return r;
}

async function typeText(env: Env, text: string) {
  for (const ch of text) env.ed.type(ch);
  await env.ed.settle();
}

export function buildScript(): Script {
  const s = new Script();

  // ---- S3: choose an action for key 7 -----------------------------------------------------------------
  s.at(6.0, "cameras", (env) => {
    // The action picker (520 wide) opens centred over the pad.
    env.cams.set("picker", frameApp({ x: APP_W / 2 - 330, y: 108, w: 660, h: 590 }, 14, 1.5));
  });
  s.camera(6.8, 7.4, "picker");
  s.moveTo(6.74, 7.06, "key7", "[data-key='KP7']", { bow: 0.12 });
  s.click(7.1, "key7");
  s.at(7.28, "enter → picker", async ({ ed }) => {
    ed.key("Enter");
    await ed.settle();
    await ed.until(() => ed.find("input[placeholder='What should this key do?']"), "action picker");
  });
  ["a", "p", "p"].forEach((ch, i) => s.at(7.42 + i * 0.07, `type ${ch}`, (env) => typeText(env, ch)));
  s.at(7.82, "enter → open an app", async (env) => {
    env.ed.key("Enter");
    await env.ed.settle();
    await env.ed.until(() => env.ed.find("button", "Choose an app"), "app field");
    const key7 = need(env, "[data-key='KP7']");
    const insp = need(env, "aside", "Appearance");
    env.cams.set("inspector", frameApp(unite(key7, { x: insp.x, y: 150, w: insp.w, h: 610 }), 14, 1.5));
  });
  s.camera(7.86, 8.36, "inspector");
  s.moveTo(7.88, 8.18, "appBtn", "button", { text: "Choose an app", ax: 0.32, bow: -0.1 });
  s.click(8.22, "appBtn", async ({ ed }) => {
    await ed.until(() => ed.find("input[placeholder='Search apps…']"), "app search");
  });
  ["s", "p", "o"].forEach((ch, i) => s.at(8.32 + i * 0.06, `type ${ch}`, (env) => typeText(env, ch)));
  s.moveTo(8.48, 8.72, "spotify", "[role='option']", { text: "Spotify", ax: 0.3, bow: 0.08 });
  s.click(8.76, "spotify", async ({ ed }) => {
    await ed.until(() => ed.find("[data-key='KP7']", "Spotify"), "key 7 label");
    // The picker resets its list as it closes; whether Radix animates that exit is a rendering-order race,
    // so close it outright (it is only ~4 frames).
    ed.finishAnimations("[data-radix-popper-content-wrapper]");
  });

  // ---- S3: drag a cover onto key 8, crop it ------------------------------------------------------------
  s.camera(8.84, 9.3, IDENTITY);
  s.at(8.86, "file point", ({ pts }) => {
    pts.set("file", screenToApp({ x: FILE.x, y: FILE.y - 8 }));
  });
  s.glide(8.86, 9.2, "file", 0.08);
  s.moveTo(DRAG.t0, DRAG.t1, "key8", "[data-key='KP8']", { bow: -0.1, dy: 6 });
  s.at(DRAG.drop - 0.12, "dragover key 8", async ({ ed, pts }) => {
    const p = pts.get("key8")!;
    await ed.fileDrag("over", p.x, p.y, FILE.url, FILE.name);
    await ed.settle();
  });
  s.at(DRAG.drop, "drop on key 8", async (env) => {
    const { ed, pts } = env;
    const p = pts.get("key8")!;
    await ed.fileDrag("drop", p.x, p.y, FILE.url, FILE.name);
    await ed.until(() => ed.find("button", "Crop"), "cover set on key 8");
    // The drop's ring changes land in one or two style passes depending on when the async import
    // finishes; settle them now so the frame is the same in every capture.
    ed.finishAnimations("[data-key='KP8']");
    await ed.settle();
    env.cams.set("crop", frameApp({ x: APP_W / 2 - 372, y: 132, w: 744, h: 548 }, 10, 1.5));
  });
  s.moveTo(9.78, 10.04, "cropBtn", "aside button", { text: "Crop", bow: 0.1 });
  s.camera(9.9, 10.4, "crop");
  s.click(10.08, "cropBtn", async ({ ed }) => {
    await ed.until(() => ed.find("[role='dialog'] .cursor-move"), "crop frame");
    await ed.images();
  });
  // Corner: drag the top-left handle in (≈1.6× zoom), then move the frame onto the sun.
  s.moveTo(10.12, 10.28, "nw", "[role='dialog'] .cursor-move > span", { bow: 0.06 });
  s.at(10.3, "nw down", ({ ed, pts, rects }) => {
    const p = pts.get("nw")!;
    const frame = ed.rect("[role='dialog'] .cursor-move")!;
    rects.set("frame0", frame);
    pts.set("nwTo", { x: p.x + frame.w * 0.375, y: p.y + frame.h * 0.375 });
    ed.pointerDown(p.x, p.y);
  });
  const drag = (t0: number, t1: number, a: string, b: string) => {
    s.glide(t0, t1, b, 0.04);
    s.during(t0, t1, ({ ed, pts }, t) => {
      const p = pts.get(a)!;
      const q = pts.get(b)!;
      const k = smooth((t - t0) / (t1 - t0));
      ed.pointerMove(lerp(p.x, q.x, k), lerp(p.y, q.y, k));
    });
  };
  drag(10.32, 10.56, "nw", "nwTo");
  s.at(10.58, "nw up", ({ ed, pts }) => {
    const p = pts.get("nwTo")!;
    ed.pointerUp(p.x, p.y);
  });
  s.at(10.59, "move frame", ({ pts, rects }) => {
    const f0 = rects.get("frame0")!;
    const p = pts.get("nwTo")!;
    const from = { x: p.x + f0.w * 0.3, y: p.y + f0.h * 0.3 };
    pts.set("mv0", from);
    pts.set("mv1", { x: from.x - f0.w * 0.1875, y: from.y - f0.h * 0.175 });
  });
  s.glide(10.59, 10.64, "mv0", 0);
  s.at(10.65, "frame down", ({ ed, pts }) => {
    const p = pts.get("mv0")!;
    ed.pointerDown(p.x, p.y);
  });
  drag(10.66, 10.86, "mv0", "mv1");
  s.at(10.88, "frame up", ({ ed, pts }) => {
    const p = pts.get("mv1")!;
    ed.pointerUp(p.x, p.y);
  });
  s.moveTo(10.9, 11.04, "save", "[role='dialog'] button", { text: "Save", bow: 0.06 });
  s.click(11.08, "save", async (env) => {
    // It animates out on the clock from here.
    await env.ed.until(() => !env.ed.find("[role='dialog'][data-state='open']"), "crop dialog closing");
  });
  s.at(11.12, "rest point", (env) => {
    const k = need(env, "[data-key='KP8']");
    env.pts.set("rest8", { x: k.x + k.w * 0.82, y: k.y + k.h * 1.08 });
  });
  s.glide(11.12, 11.5, "rest8", 0.14);
  s.at(11.0, "pad camera", (env) => {
    const k = need(env, "[data-key='KP4']");
    // Keybed in the upper part of the frame, the caption below it.
    env.cams.set("pad", frameApp({ x: 330, y: 150, w: 620, h: 640 }, 10, 1.4));
    env.cams.set("key4", frameApp({ x: k.x - k.w * 0.25, y: k.y - k.h * 0.1, w: k.w * 1.5, h: k.h * 1.2 }, 0, 3.2));
  });
  s.camera(11.06, 11.44, "pad");

  // ---- S4: flow editor ---------------------------------------------------------------------------------
  s.camera(T.pushIn[0], T.pushIn[1], "key4", easeInOut);
  s.at(T.flowSwap, "open flow editor", async (env) => {
    const { ed } = env;
    ed.editor.selectKey("KP4");
    ed.editor.openFlow(AUDIO, "KP4");
    await ed.settle();
    await ed.until(() => ed.find(".react-flow__node"), "flow nodes");
    // React Flow fits the view once nodes are measured.
    for (let i = 0; i < 6; i++) await ed.settle();
    const all = env.ed.nodesBox();
    const test = need(env, "button", "Test");
    const view = unite(all, test);
    env.cams.set("flowIn", frameApp({ x: all.x + all.w / 2 - 110, y: all.y + all.h / 2 - 80, w: 220, h: 160 }, 0, 2.6));
    // Leave room under the nodes for the caption.
    env.cams.set("flow", frameApp({ x: view.x - 30, y: view.y - 6, w: view.w + 60, h: view.h + 150 }, 8, 1.6));
    env.pts.set("test", { x: test.x + test.w * 0.45, y: test.y + test.h / 2 });
    env.cams.set("nodes", frameApp({ x: all.x - 40, y: all.y - 40, w: all.w + 80, h: all.h + 170 }, 8, 2.1));
    const code = need(env, "[role='tab']", "Code");
    env.cams.set("code", frameApp({ x: code.x - 60, y: code.y - 12, w: 720, h: 330 }, 8, 1.9));
  });
  s.camera(TEST.click + 0.02, TEST.click + 0.4, "nodes");
  s.camera(12.9, 13.3, "code");
  s.camera(T.flowSwap, T.flowSwap + 0.001, "flowIn");
  s.camera(T.flowSwap + 0.02, T.flowSwap + 0.55, "flow", easeOut);
  s.glide(12.0, 12.32, "test", -0.1);
  s.click(TEST.click, "test");
  s.at(TEST.click + 0.01, "run started", ({ ed }) => {
    ed.emit({ event: "run", data: { runId: ed.currentRunId(), key: "KP4", status: "started" } });
  });
  const trace = ["kp4_n1", "kp4_n2", "kp4_n3"];
  trace.forEach((nodeId, i) =>
    s.at(TEST.click + 0.04 + i * TEST.step, `trace ${nodeId}`, async ({ ed }) => {
      ed.emit({ event: "trace", data: { runId: ed.currentRunId(), nodeId } });
      if (i === trace.length - 1) {
        const graph = ed.useEditor.getState().config!.profiles.find((p) => p.id === AUDIO)!.keys.KP4!.macro;
        const node = graph?.type === "graph" ? graph.nodes.find((n: GraphNode) => n.id === nodeId) : undefined;
        ed.emit({ event: "log", data: { runId: ed.currentRunId(), level: "info", message: (node && actionJs(node)) || "press(\"alt+left\")" } });
      }
      await ed.settle();
    }),
  );
  s.at(TEST.click + 0.04 + trace.length * TEST.step, "run finished", async ({ ed }) => {
    ed.emit({ event: "run", data: { runId: ed.currentRunId(), key: "KP4", status: "finished" } });
    await ed.settle();
  });
  s.moveTo(12.94, 13.2, "codeTab", "[role='tab']", { text: "Code", bow: 0.1 });
  s.click(13.24, "codeTab", async ({ ed }) => {
    await ed.until(() => ed.find(".cm-content"), "code view");
    await ed.settle();
  });

  // ---- S5: pull back as the window leaves ---------------------------------------------------------------
  s.camera(T.outro, T.outro + 0.55, IDENTITY, easeInOut);

  return s.sort();
}

const smooth = (x: number) => {
  const k = Math.min(1, Math.max(0, x));
  return k * k * k * (k * (k * 6 - 15) + 10);
};
const easeInOut = (x: number) => (x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2);
const easeOut = (x: number) => 1 - Math.pow(1 - x, 4);

export function screenToApp(p: Pt): Pt {
  return { x: (p.x - WIN_X) / WIN_SCALE, y: (p.y - WIN_Y) / WIN_SCALE - TITLE_H };
}
