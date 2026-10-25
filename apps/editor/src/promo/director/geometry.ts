// Canvas, "screen" (the desktop the camera films) and the editor window's placement on it.

import { clamp, lerp, type Pt } from "../anim";

export const W = 1280;
export const H = 720;
/** Timeline sampling rate. Frames are played back at 30 fps (promo/encode.sh), so sampling the 15 s
 *  timeline at 40 fps gives a 20 s promo at 75 % speed without dropping smoothness. */
export const FPS = 40;
export const FRAMES = 600;

/** The editor window: a 1280×800 app viewport under a slim title bar, scaled onto the 1280×720 screen. */
export const APP_W = 1280;
export const APP_H = 800;
export const TITLE_H = 30;
export const WIN_SCALE = 0.82;
export const WIN_X = (W - APP_W * WIN_SCALE) / 2;
export const WIN_Y = (H - (APP_H + TITLE_H) * WIN_SCALE) / 2;

/** App viewport point → screen point. */
export function appToScreen(p: Pt): Pt {
  return { x: WIN_X + p.x * WIN_SCALE, y: WIN_Y + (TITLE_H + p.y) * WIN_SCALE };
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Camera: screen point (cx, cy) sits at the canvas centre, magnified z×. */
export interface Camera {
  cx: number;
  cy: number;
  z: number;
}

export const IDENTITY: Camera = { cx: W / 2, cy: H / 2, z: 1 };

/** Frame an app-viewport rect (with `pad` screen px around it), never showing past the screen's edges. */
export function frameApp(r: Rect, pad = 24, maxZ = 1.6): Camera {
  const a = appToScreen({ x: r.x, y: r.y });
  const b = appToScreen({ x: r.x + r.w, y: r.y + r.h });
  const z = clamp(Math.min(W / (b.x - a.x + pad * 2), H / (b.y - a.y + pad * 2)), 1, maxZ);
  return fit({ cx: (a.x + b.x) / 2, cy: (a.y + b.y) / 2, z });
}

export function fit(c: Camera): Camera {
  const hw = W / 2 / c.z;
  const hh = H / 2 / c.z;
  return { z: c.z, cx: clamp(c.cx, hw, W - hw), cy: clamp(c.cy, hh, H - hh) };
}

/** Interpolate cameras: centre linearly, zoom in log space (constant perceived speed). */
export function mixCamera(a: Camera, b: Camera, k: number): Camera {
  return fit({ cx: lerp(a.cx, b.cx, k), cy: lerp(a.cy, b.cy, k), z: Math.exp(lerp(Math.log(a.z), Math.log(b.z), k)) });
}

export function cameraTransform(c: Camera): string {
  return `translate(${W / 2 - c.cx * c.z}px, ${H / 2 - c.cy * c.z}px) scale(${c.z})`;
}
