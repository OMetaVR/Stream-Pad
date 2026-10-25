// S1 — "Your numpad → a macropad": the editor's real keycaps in CSS 3D. A Scroll Lock press lights its LED
// and a wave flips every key from its plain legend to the Audio profile's face.

import { KEYCAP_CLASS, KEYCAP_SHADOW, KeycapFace, PAD_GEOMETRY } from "@/features/numpad/NumpadCanvas";
import { cn } from "@/lib/cn";
import { KEY_IDS, KEY_INFO, type KeyId } from "@/lib/types";
import { at, clamp01, ease, lerp } from "../anim";
import { profileFaces } from "./faces";
import { T } from "./script";

const { unit: U, gap: G, pad: P } = PAD_GEOMETRY;
const BED_W = 4 * U + 3 * G + 2 * P;
const BED_H = 5 * U + 4 * G + 2 * P;
const AUDIO = profileFaces("Audio");
const ACCENT = "#8b7cf6";
const CAP_Z = 12;
const FLIP = 0.3;
const flipEase = ease.outBack(0.04);

/** Scroll Lock plate position relative to the keybed's top-left. */
const SL = { x: -U - P * 2 - 16, y: -U - P * 2 - 14 };

function centre(k: KeyId) {
  const g = KEY_INFO[k].grid;
  return { x: P + g.col * (U + G) + (g.colSpan * U + (g.colSpan - 1) * G) / 2, y: P + g.row * (U + G) + (g.rowSpan * U + (g.rowSpan - 1) * G) / 2 };
}

// Wave order: distance from Scroll Lock.
const slCentre = { x: SL.x + P + U / 2, y: SL.y + P + U / 2 };
const dist = Object.fromEntries(KEY_IDS.map((k) => [k, Math.hypot(centre(k).x - slCentre.x, centre(k).y - slCentre.y)])) as Record<KeyId, number>;
const dMin = Math.min(...Object.values(dist));
const dMax = Math.max(...Object.values(dist));
const delayOf = (k: KeyId) => ((dist[k] - dMin) / (dMax - dMin)) * (T.wave[1] - T.wave[0] - FLIP);

/** The keycap's sides: stacked slices, darker towards the plate. */
function Skirt({ depth = CAP_Z }: { depth?: number }) {
  const n = Math.max(1, Math.ceil(depth / 1.5));
  return (
    <>
      {Array.from({ length: n }, (_, i) => (
        <div
          key={i}
          className="absolute inset-0 rounded-[11px]"
          style={{ transform: `translateZ(${(i * depth) / n}px)`, background: `color-mix(in srgb, #0b0c0f, #23252d ${Math.round(25 + (60 * i) / n)}%)` }}
        />
      ))}
    </>
  );
}

/** Plate thickness under a keybed-style element. */
function Slab({ depth = 14, radius = 16 }: { depth?: number; radius?: number }) {
  return (
    <>
      {Array.from({ length: Math.ceil(depth / 2) }, (_, i) => (
        <div key={i} className="absolute inset-0" style={{ borderRadius: radius, transform: `translateZ(${-depth + i * 2}px)`, background: i === 0 ? "#08090b" : "#0d0e12" }} />
      ))}
    </>
  );
}

function Key3D({ k, t }: { k: KeyId; t: number }) {
  const g = KEY_INFO[k].grid;
  const p = clamp01((t - T.wave[0] - delayOf(k)) / FLIP);
  const angle = 180 * flipEase(p);
  const bump = Math.sin(Math.PI * clamp01(p * 1.1));
  const f = AUDIO[k];
  return (
    <div
      style={{
        gridColumn: `${g.col + 1} / span ${g.colSpan}`,
        gridRow: `${g.row + 1} / span ${g.rowSpan}`,
        transformStyle: "preserve-3d",
      }}
      className="relative"
    >
      <Skirt />
      <div className="absolute inset-0" style={{ transformStyle: "preserve-3d", transform: `translateZ(${CAP_Z + 16 * bump}px) rotateX(${angle}deg) scale(${1 + 0.04 * bump})` }}>
        <div className={cn(KEYCAP_CLASS, KEYCAP_SHADOW, "absolute inset-0")} style={{ backfaceVisibility: "hidden" }}>
          <KeycapFace keyId={k} empty />
        </div>
        <div className={cn(KEYCAP_CLASS, KEYCAP_SHADOW, "absolute inset-0")} style={{ backfaceVisibility: "hidden", transform: "rotateX(180deg)" }}>
          <KeycapFace keyId={k} label={f.label} icon={f.icon} cover={f.cover} coverCrop={f.coverCrop} global={f.global} empty={f.empty} />
        </div>
      </div>
    </div>
  );
}

function ScrollLock({ t }: { t: number }) {
  const down = at(t, T.scrollLock, T.scrollLock + 0.05, ease.outCubic) * (1 - at(t, T.scrollLock + 0.09, T.scrollLock + 0.2, ease.outCubic));
  const led = at(t, T.scrollLock + 0.04, T.scrollLock + 0.16, ease.outCubic);
  const bloom = led * (1 + 0.35 * Math.exp(-Math.max(0, t - T.scrollLock - 0.12) * 6));
  return (
    <div className="absolute" style={{ left: SL.x, top: SL.y, width: U + 2 * P, height: U + 2 * P, transformStyle: "preserve-3d" }}>
      <Slab />
      <div className="absolute inset-0 rounded-xl border border-line bg-keybed shadow-raised" />
      <div className="absolute" style={{ inset: P, transformStyle: "preserve-3d" }}>
        <Skirt depth={CAP_Z - 3 * down} />
        <div
          className={cn(KEYCAP_CLASS, "absolute inset-0")}
          style={{
            transform: `translateZ(${CAP_Z - 3.5 * down}px)`,
            boxShadow: `inset 0 1px 0 var(--sp-keycap-highlight), 0 ${3 - 2 * down}px 0 var(--sp-keycap-skirt)`,
          }}
        >
          <span className="absolute left-2 top-1.5 flex items-baseline gap-1 leading-none">
            <span className="text-sm font-semibold text-muted">Scroll</span>
            <span className="text-2xs text-faint">Lock</span>
          </span>
          <span
            className="absolute bottom-3 right-3 size-[7px] rounded-full"
            style={{
              background: led > 0 ? `color-mix(in srgb, ${ACCENT} ${Math.round(led * 100)}%, #2a2c34)` : "#2a2c34",
              boxShadow: led > 0 ? `0 0 ${4 + 6 * bloom}px ${1 + 2 * bloom}px color-mix(in srgb, ${ACCENT} ${Math.round(70 * led)}%, transparent), 0 0 ${18 * bloom}px ${6 * bloom}px color-mix(in srgb, ${ACCENT} ${Math.round(28 * led)}%, transparent)` : "inset 0 1px 1px rgba(0,0,0,.6)",
            }}
          />
        </div>
      </div>
    </div>
  );
}

function Caption({ t }: { t: number }) {
  const a = at(t, T.caption1, T.caption1 + 0.55, ease.outCubic);
  const b = at(t, T.caption2, T.caption2 + 0.55, ease.outCubic);
  const out = at(t, 2.04, 2.36, ease.inCubic);
  const lh = 64;
  return (
    <div className="absolute" style={{ left: 104, top: 360, opacity: 1 - out, transform: `translateY(${-8 * out}px)` }}>
      <div style={{ fontSize: 56, lineHeight: `${lh}px`, fontWeight: 600, letterSpacing: "-0.02em", color: "#e9e9ef", whiteSpace: "nowrap" }}>
        <div style={{ opacity: a, transform: `translateY(${-lh / 2 - lh / 2 * b + 18 * (1 - a)}px)`, filter: `blur(${8 * (1 - a)}px)` }}>Your numpad.</div>
        <div style={{ position: "absolute", top: 0, opacity: b, transform: `translateY(${lh / 2 - lh / 2 + 18 * (1 - b)}px)`, filter: `blur(${8 * (1 - b)}px)`, color: "#b3a6ff" }}>Now a macropad.</div>
      </div>
    </div>
  );
}

export function Intro({ t }: { t: number }) {
  if (t > T.flatten[1] + 0.05) return null;
  const fade = at(t, T.fadeUp[0], T.fadeUp[1], ease.outCubic);
  const k = at(t, 0, T.flatten[0], ease.inOutSine);
  const fl = at(t, T.flatten[0], T.flatten[1], ease.inOutCubic);
  const rx = lerp(lerp(50, 38, k), 0, fl);
  const rz = lerp(lerp(-16, -10, k), 0, fl);
  // The pad flattens and heads for the screen's corner — where the on-screen pad will slide in.
  const scale = lerp(lerp(1.06, 1.12, k), 0.5, fl);
  const dx = lerp(0, 330, fl);
  const dy = lerp(lerp(14, 0, k), 200, fl);
  const opacity = fade * (1 - at(t, T.flatten[0] + 0.12, T.flatten[1], ease.inQuad));
  return (
    <div className="absolute inset-0" style={{ opacity }}>
      <div className="absolute inset-0" style={{ perspective: 1400, perspectiveOrigin: "62% 40%" }}>
        <div
          className="absolute"
          style={{
            left: 820 - BED_W / 2,
            top: 372 - BED_H / 2,
            width: BED_W,
            height: BED_H,
            transformStyle: "preserve-3d",
            transform: `translate(${dx}px, ${dy}px) rotateX(${rx}deg) rotateZ(${rz}deg) scale(${scale})`,
          }}
        >
          {/* soft contact shadow */}
          <div className="absolute rounded-[40px] bg-black" style={{ inset: -10, transform: "translateZ(-30px) translateY(26px)", filter: "blur(28px)", opacity: 0.75 * (1 - fl) }} />
          <ScrollLock t={t} />
          <Slab />
          <div
            className="grid rounded-xl border border-line bg-keybed shadow-raised"
            style={{ padding: P, gridTemplateColumns: `repeat(4, ${U}px)`, gridTemplateRows: `repeat(5, ${U}px)`, gap: G, transformStyle: "preserve-3d" }}
          >
            {KEY_IDS.map((key) => (
              <Key3D key={key} k={key} t={t} />
            ))}
          </div>
        </div>
      </div>
      <Caption t={t} />
    </div>
  );
}
