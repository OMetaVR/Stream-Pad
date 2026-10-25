// S2–S4: the "screen" the camera films — wallpaper, a blurred app, the real overlay's frames composited at
// the bottom-right corner, a desktop file, the editor window (real app in an iframe) and the cursor.

import { forwardRef } from "react";
import { KEYCAP_CLASS, KEYCAP_SHADOW, KeycapFace, PAD_GEOMETRY } from "@/features/numpad/NumpadCanvas";
import { cn } from "@/lib/cn";
import { KEY_IDS, KEY_INFO } from "@/lib/types";
import { at, ease, lerp, window4, type Pt } from "../anim";
import { profileFaces } from "./faces";
import { APP_H, APP_W, H, TITLE_H, W, WIN_SCALE, WIN_X, WIN_Y, type Camera, cameraTransform } from "./geometry";
import { DRAG, FILE, T } from "./script";

export interface OverlaySeq {
  base: string;
  frames: number;
  fps: number;
  width: number;
  height: number;
  anchor: { x: number; y: number };
}

function Wallpaper({ dim }: { dim: number }) {
  return (
    <div className="absolute inset-0 overflow-hidden" style={{ background: "#0b0d1a" }}>
      <div
        className="absolute inset-0"
        style={{
          background: [
            "radial-gradient(60% 75% at 18% 22%, rgba(54, 74, 168, 0.55), transparent 70%)",
            "radial-gradient(55% 70% at 82% 30%, rgba(124, 82, 214, 0.42), transparent 72%)",
            "radial-gradient(70% 60% at 62% 92%, rgba(186, 72, 168, 0.22), transparent 70%)",
            "radial-gradient(45% 50% at 38% 70%, rgba(40, 118, 196, 0.25), transparent 70%)",
            "linear-gradient(160deg, #0d1230 0%, #120f2a 48%, #0a0a16 100%)",
          ].join(","),
        }}
      />
      <div className="absolute inset-0" style={{ background: "radial-gradient(120% 90% at 50% 40%, transparent 55%, rgba(4,5,10,0.55) 100%)" }} />
      <div className="absolute inset-0 bg-black" style={{ opacity: dim }} />
    </div>
  );
}

/** An out-of-focus music player — context, not content. */
function BlurredApp({ opacity }: { opacity: number }) {
  if (opacity <= 0) return null;
  return (
    <div className="absolute" style={{ left: 120, top: 92, width: 760, height: 470, opacity: 0.7 * opacity, filter: "blur(10px)" }}>
      <div className="absolute inset-0 rounded-[14px]" style={{ background: "#14151d", boxShadow: "0 30px 80px rgba(0,0,0,.5)" }}>
        <div className="absolute left-0 top-0 h-9 w-full rounded-t-[14px]" style={{ background: "#1a1b24" }} />
        <div className="absolute left-0 top-9 bottom-0 w-44" style={{ background: "#111218" }} />
        <div className="absolute rounded-[10px]" style={{ left: 210, top: 70, width: 190, height: 190, background: "linear-gradient(140deg,#ff7a59,#b04fd6 55%,#3a3fd0)" }} />
        <div className="absolute rounded" style={{ left: 424, top: 110, width: 240, height: 26, background: "#e9e9ef", opacity: 0.8 }} />
        <div className="absolute rounded" style={{ left: 424, top: 152, width: 160, height: 16, background: "#8b8d98", opacity: 0.7 }} />
        <div className="absolute rounded-full" style={{ left: 424, top: 200, width: 64, height: 64, background: "#3fcf8e" }} />
        {Array.from({ length: 5 }, (_, i) => (
          <div key={i} className="absolute rounded" style={{ left: 210, top: 300 + i * 30, width: 470 - (i % 2) * 90, height: 12, background: "#2a2c38" }} />
        ))}
        <div className="absolute bottom-0 left-0 h-16 w-full rounded-b-[14px]" style={{ background: "#1a1b24" }} />
        <div className="absolute rounded-full" style={{ left: 260, bottom: 28, width: 360, height: 6, background: "#2e3040" }} />
        <div className="absolute rounded-full" style={{ left: 260, bottom: 28, width: 150, height: 6, background: "#3fcf8e" }} />
      </div>
    </div>
  );
}

const AUDIO = profileFaces("Audio");
const APPS = profileFaces("Apps");

/** Stand-in for the overlay until its real frames exist (promo/build/overlay/s2). */
function PlaceholderOverlay({ t }: { t: number }) {
  const { unit: U0, gap: G0 } = PAD_GEOMETRY;
  const U = 58;
  const G = 8;
  const pw = 4 * U + 3 * G + 32;
  const ph = 5 * U + 4 * G + 72;
  const inK = at(t, 2.7, 3.02, ease.outQuint);
  const outK = at(t, 6.1, 6.38, ease.inCubic);
  const x = lerp(pw + 20, 0, inK) + lerp(0, pw + 20, outK);
  const apps = at(t, 4.5, 4.72, ease.inOutCubic);
  void U0;
  void G0;
  if (inK <= 0 || outK >= 1) return null;
  const faces = (f: typeof AUDIO, opacity: number) => (
    <div className="absolute grid" style={{ left: 16, top: 56, gridTemplateColumns: `repeat(4, ${U}px)`, gridTemplateRows: `repeat(5, ${U}px)`, gap: G, opacity }}>
      {KEY_IDS.map((k) => {
        const g = KEY_INFO[k].grid;
        const press = (k === "KP5" && window4(t, 3.7, 3.74, 3.86, 4.1)) || (k === "KP1" && window4(t, 5.3, 5.34, 5.46, 5.7)) || 0;
        return (
          <div
            key={k}
            className={cn(KEYCAP_CLASS, KEYCAP_SHADOW, "pt-2")}
            style={{ gridColumn: `${g.col + 1} / span ${g.colSpan}`, gridRow: `${g.row + 1} / span ${g.rowSpan}`, boxShadow: press ? `0 0 0 ${3 * press}px rgba(139,124,246,.5), inset 0 1px 0 var(--sp-keycap-highlight)` : undefined }}
          >
            <KeycapFace keyId={k} {...f[k]} />
          </div>
        );
      })}
    </div>
  );
  return (
    <div className="absolute overflow-hidden rounded-tl-[22px] border-l border-t border-line-strong" style={{ right: 0, bottom: 0, width: pw, height: ph, transform: `translateX(${x}px)`, background: "rgba(18,19,24,0.94)" }}>
      <div className="absolute left-4 top-4 text-md font-semibold text-fg">{apps > 0.5 ? "Apps" : "Audio"}</div>
      <div className="absolute right-4 top-5 flex gap-1.5">
        {[0, 1, 2, 3, 4].map((i) => (
          <span key={i} className="h-1.5 rounded-full" style={{ width: i === (apps > 0.5 ? 1 : 0) ? 14 : 6, background: i === (apps > 0.5 ? 1 : 0) ? (apps > 0.5 ? "#4aa8ff" : "#8b7cf6") : "#3a3c46" }} />
        ))}
      </div>
      {faces(AUDIO, 1 - apps)}
      {apps > 0 && faces(APPS, apps)}
      {window4(t, 3.72, 3.85, 4.35, 4.55) > 0 && (
        <div className="absolute left-1/2 top-[46%] -translate-x-1/2 rounded-full border border-line-strong bg-raised px-4 py-2 text-base font-medium text-fg shadow-popover" style={{ opacity: window4(t, 3.72, 3.85, 4.35, 4.55) }}>
          Play / Pause
        </div>
      )}
    </div>
  );
}

function Overlay({ t, seq }: { t: number; seq: OverlaySeq | null }) {
  if (!seq) return <PlaceholderOverlay t={t} />;
  const i = Math.round((t - T.s2) * seq.fps);
  if (i < 0 || i >= seq.frames) return null;
  return (
    <img
      src={`${seq.base}/${String(i).padStart(4, "0")}.png`}
      alt=""
      className="absolute max-w-none"
      style={{ left: W - seq.anchor.x, top: H - seq.anchor.y, width: seq.width, height: seq.height }}
    />
  );
}

function TitleBar() {
  return (
    <div className="relative flex items-center justify-center" style={{ height: TITLE_H, background: "#0f1014", borderBottom: "1px solid rgba(255,255,255,0.06)" }}>
      <span className="text-sm font-medium text-muted">StreamPad</span>
      <div className="absolute right-3 top-0 flex h-full items-center gap-4 text-faint">
        <svg width="10" height="10" viewBox="0 0 10 10"><path d="M1 5.5h8" stroke="currentColor" strokeWidth="1.3" /></svg>
        <svg width="10" height="10" viewBox="0 0 10 10"><path d="M5 1.5 8.5 5 5 8.5 1.5 5Z" fill="none" stroke="currentColor" strokeWidth="1.2" /></svg>
        <svg width="10" height="10" viewBox="0 0 10 10"><path d="m1.5 1.5 7 7m0-7-7 7" stroke="currentColor" strokeWidth="1.3" /></svg>
      </div>
    </div>
  );
}

export const EditorWindow = forwardRef<HTMLIFrameElement, { t: number }>(function EditorWindow({ t }, ref) {
  const rise = at(t, T.rise[0], T.rise[1], ease.outQuint);
  const out = at(t, T.outro, T.outro + 0.45, ease.inOutCubic);
  const visible = rise > 0 && out < 1;
  const s = WIN_SCALE * lerp(1, 0.96, at(t, T.outro, T.outro + 0.5, ease.inOutCubic));
  const cx = WIN_X + (APP_W * WIN_SCALE) / 2;
  const cy = WIN_Y + ((APP_H + TITLE_H) * WIN_SCALE) / 2;
  return (
    <div
      className="absolute left-0 top-0"
      style={{
        width: APP_W,
        height: APP_H + TITLE_H,
        transformOrigin: "0 0",
        transform: `translate(${cx - (APP_W * s) / 2}px, ${cy - ((APP_H + TITLE_H) * s) / 2 + 40 * (1 - rise)}px) scale(${s})`,
        // Never visibility:hidden — a hidden iframe gets throttled (no ResizeObserver/rAF) and would jump on show.
        opacity: visible ? rise * (1 - out) : 0,
      }}
    >
      <div className="absolute rounded-[16px]" style={{ inset: 0, boxShadow: "0 40px 120px -20px rgba(0,0,0,0.85), 0 18px 40px -12px rgba(0,0,0,0.6)" }} />
      <div className="relative overflow-hidden rounded-[14px]" style={{ boxShadow: "0 0 0 1px rgba(255,255,255,0.09)" }}>
        <TitleBar />
        <iframe ref={ref} title="StreamPad editor" src="/promo.html?part=editor" width={APP_W} height={APP_H} className="block border-0" style={{ background: "#0c0d10" }} />
      </div>
    </div>
  );
});

function DesktopFile({ t }: { t: number }) {
  const show = window4(t, 6.4, 6.85, T.outro, T.outro + 0.3);
  if (show <= 0) return null;
  const dragging = t >= DRAG.press && t < DRAG.drop + 0.05;
  const selected = t >= DRAG.press - 0.04;
  return (
    <div className="absolute flex flex-col items-center gap-1.5" style={{ left: FILE.x - 50, top: FILE.y - 36, width: 100, opacity: show }}>
      <div className="rounded-[8px] p-1" style={{ background: selected ? "rgba(139,124,246,0.28)" : "transparent" }}>
        <img src={FILE.url} alt="" className="block size-14 rounded-[5px]" style={{ opacity: dragging ? 0.55 : 1, boxShadow: "0 4px 14px rgba(0,0,0,.5)" }} />
      </div>
      <span className="rounded-[4px] px-1 text-xs font-medium text-white" style={{ textShadow: "0 1px 3px rgba(0,0,0,.9)", background: selected ? "rgba(139,124,246,0.55)" : "transparent" }}>
        {FILE.name}
      </span>
    </div>
  );
}

const ARROW = "M1 1 L1 19.2 L5.6 14.9 L8.9 22.3 L12.1 20.9 L8.9 13.7 L15.1 13.7 Z";

function Cursor({ t, p, clicks, opacity }: { t: number; p: Pt; clicks: number[]; opacity: number }) {
  if (opacity <= 0) return null;
  let press = 0;
  let ripple: { k: number } | null = null;
  for (const c of clicks) {
    press = Math.max(press, window4(t, c - 0.06, c, c + 0.02, c + 0.14, ease.outCubic, ease.outCubic));
    if (t >= c && t < c + 0.42) ripple = { k: (t - c) / 0.42 };
  }
  const dragging = t >= DRAG.press && t < DRAG.drop;
  const drop = at(t, DRAG.drop, DRAG.drop + 0.14, ease.outCubic);
  const chip = t >= DRAG.press && drop < 1;
  const scale = 1 - 0.1 * press;
  return (
    <div className="pointer-events-none absolute left-0 top-0" style={{ transform: `translate(${p.x}px, ${p.y}px)`, opacity }}>
      {ripple && (
        <div
          className="absolute rounded-full"
          style={{
            left: -22,
            top: -22,
            width: 44,
            height: 44,
            border: "2px solid rgba(179,166,255,0.9)",
            transform: `scale(${lerp(0.25, 1, ease.outCubic(ripple.k))})`,
            opacity: 0.8 * (1 - ease.inQuad(ripple.k)),
          }}
        />
      )}
      {chip && (
        <div
          className="absolute flex items-center gap-2 rounded-[8px] border border-line-strong bg-raised py-1 pl-1 pr-2.5 shadow-popover"
          style={{ left: 14, top: 18, opacity: (dragging ? at(t, DRAG.press, DRAG.press + 0.1, ease.outCubic) : 1) * (1 - drop) * 0.96, transform: `scale(${lerp(1, 0.7, drop)})`, transformOrigin: "0 0" }}
        >
          <img src={FILE.url} alt="" className="block size-8 rounded-[5px]" />
          <span className="text-sm font-medium text-fg">{FILE.name}</span>
        </div>
      )}
      <svg width="24" height="26" viewBox="0 0 24 26" className="absolute" style={{ left: -1, top: -1, transform: `scale(${scale})`, transformOrigin: "2px 2px", overflow: "visible", filter: "drop-shadow(0 2px 3px rgba(0,0,0,0.45))" }}>
        <path d={ARROW} fill="#fff" stroke="#111216" strokeWidth="1.4" strokeLinejoin="round" />
      </svg>
    </div>
  );
}

export interface ScreenProps {
  t: number;
  camera: Camera;
  overlay: OverlaySeq | null;
  cursor: Pt;
  cursorOpacity: number;
  clicks: number[];
  flowDip: number;
  iframeRef: React.Ref<HTMLIFrameElement>;
}

export function Screen({ t, camera, overlay, cursor, cursorOpacity, clicks, flowDip, iframeRef }: ScreenProps) {
  const enter = at(t, 1.98, 2.46, ease.inOutCubic);
  const exit = at(t, T.outro + 0.05, T.outro + 0.5, ease.inOutCubic);
  const dim = lerp(0, 0.5, at(t, T.dim[0], T.dim[1], ease.inOutCubic));
  const opacity = enter * (1 - exit);
  const settle = lerp(1.05, 1, enter);
  return (
    <div className="absolute inset-0 overflow-hidden" style={{ opacity }}>
      <div className="absolute left-0 top-0" style={{ width: W, height: H, transformOrigin: "0 0", transform: `${cameraTransform(camera)} translate(${W / 2}px, ${H / 2}px) scale(${settle}) translate(${-W / 2}px, ${-H / 2}px)` }}>
        <Wallpaper dim={dim} />
        <BlurredApp opacity={1 - at(t, T.dim[0], T.dim[1], ease.inOutCubic)} />
        <Overlay t={t} seq={overlay} />
        <DesktopFile t={t} />
        <div style={{ opacity: 1 - flowDip }}>
          <EditorWindow ref={iframeRef} t={t} />
        </div>
        <Cursor t={t} p={cursor} clicks={clicks} opacity={cursorOpacity} />
      </div>
    </div>
  );
}
