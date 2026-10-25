// The promo, as a pure function of time `t` (+ points measured from the real UI by the script's events).

import { LucideProvider } from "lucide-react";
import { ease, window4 } from "../anim";
import type { Env, Script } from "./engine";
import { Captions, KeyCast, Outro, Stage } from "./Hud";
import { Intro } from "./Intro";
import { appToScreen, H, IDENTITY, W } from "./geometry";
import { Screen, type OverlaySeq } from "./Screen";
import { CURSOR_SHOW, CURSOR_START, T } from "./script";

export interface DirectorProps {
  t: number;
  env: Env | null;
  script: Script;
  overlay: OverlaySeq | null;
  iframeRef: React.Ref<HTMLIFrameElement>;
}

export function Director({ t, env, script, overlay, iframeRef }: DirectorProps) {
  const camera = env ? script.cameraAt(t, env, IDENTITY) : IDENTITY;
  const cursorApp = env ? script.cursor(t, env, CURSOR_START) : CURSOR_START;
  const cursor = appToScreen(cursorApp);
  const cursorOpacity = window4(t, ...CURSOR_SHOW);
  // Push-in into the flow editor: dip the window to the stage at the swap.
  const flowDip = 0.88 * window4(t, T.pushIn[1] - 0.09, T.flowSwap, T.flowSwap + 0.01, T.flowSwap + 0.16, ease.inQuad, ease.outCubic);
  return (
    <LucideProvider size={16} strokeWidth={1.75}>
      <div className="relative overflow-hidden text-fg" style={{ width: W, height: H, background: "#0a0b0e" }}>
        <Stage t={t} />
        <Intro t={t} />
        <Screen t={t} camera={camera} overlay={overlay} cursor={cursor} cursorOpacity={cursorOpacity} clicks={script.clicks} flowDip={flowDip} iframeRef={iframeRef} />
        <KeyCast t={t} />
        <Captions t={t} />
        <Outro t={t} />
      </div>
    </LucideProvider>
  );
}
