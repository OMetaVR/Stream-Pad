// Canvas-space overlays: the stage, caption pills, the key-cast HUD and the outro.

import { KEYCAP_SHADOW } from "@/features/numpad/NumpadCanvas";
import { cn } from "@/lib/cn";
import { at, ease, lerp, window4 } from "../anim";
import { CAPTIONS, KEYCAST, T } from "./script";

const TAU = Math.PI * 2;

/** Near-black graphite with two soft glows drifting on a 15 s loop, and a gentle vignette. */
export function Stage({ t }: { t: number }) {
  const a = (t / T.end) * TAU;
  const g1 = { x: 58 + 7 * Math.cos(a), y: 44 + 6 * Math.sin(a) };
  const g2 = { x: 26 + 6 * Math.cos(a + 2.1), y: 70 + 6 * Math.sin(a + 2.1) };
  return (
    <div className="absolute inset-0" style={{ background: "#0a0b0e" }}>
      <div className="absolute inset-0" style={{ background: `radial-gradient(48% 62% at ${g1.x}% ${g1.y}%, rgba(139,124,246,0.14), transparent 70%)` }} />
      <div className="absolute inset-0" style={{ background: `radial-gradient(44% 58% at ${g2.x}% ${g2.y}%, rgba(74,168,255,0.08), transparent 70%)` }} />
      <div className="absolute inset-0" style={{ background: "radial-gradient(130% 100% at 50% 45%, transparent 55%, rgba(0,0,0,0.5) 100%)" }} />
    </div>
  );
}

function rich(text: string) {
  return text.split(/(\{[^}]+\})/).map((part, i) =>
    part.startsWith("{") ? (
      <span key={i} style={{ color: "#b3a6ff" }}>
        {part.slice(1, -1)}
      </span>
    ) : (
      <span key={i}>{part}</span>
    ),
  );
}

export function Captions({ t }: { t: number }) {
  return (
    <>
      {CAPTIONS.map((c) => {
        const k = window4(t, c.t0, c.t0 + 0.32, c.t1 - 0.2, c.t1, ease.outCubic, ease.inCubic);
        if (k <= 0) return null;
        const entering = t < c.t0 + 0.32;
        const tl = c.pos === "tl";
        const dy = entering ? lerp(tl ? -10 : 10, 0, k) : lerp(tl ? 6 : -6, 0, k);
        return (
          <div
            key={c.t0}
            className="absolute flex items-center rounded-[18px]"
            style={{
              ...(tl ? { left: 40, top: 36 } : { left: "50%", bottom: 34 }),
              height: 64,
              padding: "0 26px",
              background: "rgba(18,19,24,0.72)",
              boxShadow: "0 0 0 1px rgba(255,255,255,0.09), 0 18px 50px -12px rgba(0,0,0,0.6)",
              backdropFilter: "blur(18px) saturate(1.2)",
              opacity: k,
              transform: `${tl ? "" : "translateX(-50%) "}translateY(${dy}px) scale(${lerp(0.98, 1, k)})`,
              transformOrigin: tl ? "0 50%" : "50% 50%",
            }}
          >
            <span style={{ fontSize: 30, fontWeight: 600, letterSpacing: "-0.02em", color: "#e9e9ef", whiteSpace: "nowrap" }}>{rich(c.text)}</span>
          </div>
        );
      })}
    </>
  );
}

/** Physical key presses, shown as keycaps bottom-left. */
export function KeyCast({ t }: { t: number }) {
  const active = KEYCAST.filter((k) => t >= k.t && t < k.t + 0.78);
  return (
    <div className="absolute bottom-10 left-10 flex items-end gap-3">
      {active.map((k) => {
        const pop = at(t, k.t, k.t + 0.16, ease.outBack(0.04));
        const fade = 1 - at(t, k.t + 0.6, k.t + 0.76, ease.inCubic);
        return (
          <div key={k.t} style={{ opacity: Math.min(1, pop * 1.2) * fade, transform: `scale(${lerp(0.8, 1, pop)}) translateY(${4 * (1 - fade)}px)`, transformOrigin: "50% 100%" }}>
            <div className={cn("flex h-16 min-w-16 items-center justify-center rounded-[12px] bg-keycap px-5 text-fg", KEYCAP_SHADOW)} style={{ fontSize: 22, fontWeight: 600, boxShadow: "inset 0 1px 0 var(--sp-keycap-highlight), 0 3px 0 var(--sp-keycap-skirt), 0 14px 30px -8px rgba(0,0,0,.6), 0 0 0 1px rgba(255,255,255,0.06)" }}>
              {k.label}
            </div>
          </div>
        );
      })}
    </div>
  );
}

/** S5 — app icon, wordmark, version line; then back to the empty stage for the loop. */
export function Outro({ t }: { t: number }) {
  if (t < T.outro) return null;
  const out = 1 - at(t, 14.62, 15.0, ease.inOutCubic);
  const item = (i: number) => at(t, T.outro + 0.1 + i * 0.08, T.outro + 0.55 + i * 0.08, ease.outQuint);
  const a = item(0);
  const b = item(1);
  const c = item(2);
  return (
    <div className="absolute inset-0 flex flex-col items-center justify-center" style={{ opacity: out }}>
      <div className="relative" style={{ opacity: a, transform: `translateY(${lerp(16, 0, a)}px) scale(${lerp(0.88, 1, a)})` }}>
        <div className="absolute rounded-full" style={{ inset: -40, background: "radial-gradient(closest-side, rgba(139,124,246,0.28), transparent)" }} />
        <img src="/__promo/icon.svg" onError={(e) => void (e.currentTarget.src = "/favicon.svg")} alt="" width={112} height={112} className="relative block" />
      </div>
      <div style={{ marginTop: 22, fontSize: 56, lineHeight: "64px", fontWeight: 650, letterSpacing: "-0.025em", color: "#e9e9ef", opacity: b, transform: `translateY(${lerp(14, 0, b)}px)` }}>StreamPad</div>
      <div style={{ marginTop: 6, fontSize: 16, lineHeight: "24px", fontWeight: 500, color: "#8b8d98", opacity: c, transform: `translateY(${lerp(10, 0, c)}px)` }}>v2 · for Linux</div>
    </div>
  );
}
