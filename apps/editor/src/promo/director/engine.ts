// Timeline engine: discrete events (applied in order as their time passes — capture only seeks forward),
// continuous per-frame hooks, a cursor path, camera moves and click marks. All motion is a pure function
// of time plus points measured from the real UI at the moment an event runs.

import { arc, at, ease, type Ease, type Pt } from "../anim";
import type { PromoEditorApi } from "../editor/api";
import { appToScreen, IDENTITY, mixCamera, type Camera, type Rect } from "./geometry";

export interface Env {
  ed: PromoEditorApi;
  /** Named points in app-viewport coordinates (cursor targets). */
  pts: Map<string, Pt>;
  /** Named cameras (computed from measured rects). */
  cams: Map<string, Camera>;
  /** Named rects in app coordinates. */
  rects: Map<string, Rect>;
}

interface Ev {
  t: number;
  label: string;
  run(env: Env): unknown;
}
interface Cont {
  t0: number;
  t1: number;
  run(env: Env, t: number): unknown;
}
interface Seg {
  t0: number;
  t1: number;
  to: string;
  bow: number;
}
interface CamKey {
  t0: number;
  t1: number;
  to: string | Camera;
  ease: Ease;
}

export class Script {
  readonly events: Ev[] = [];
  readonly conts: Cont[] = [];
  readonly segs: Seg[] = [];
  readonly camKeys: CamKey[] = [];
  readonly clicks: number[] = [];

  at(t: number, label: string, run: (env: Env) => unknown): this {
    this.events.push({ t, label, run });
    return this;
  }

  /** Per-frame hook over [t0, t1]; also runs once at t1 so sparse previews (--step) end in the same state. */
  during(t0: number, t1: number, run: (env: Env, t: number) => unknown): this {
    this.conts.push({ t0, t1, run });
    return this.at(t1, "during end", (env) => run(env, t1));
  }

  /** Cursor glides to point `name` over [t0, t1] (the point must be known by t0). */
  glide(t0: number, t1: number, name: string, bow = 0.1): this {
    this.segs.push({ t0, t1, to: name, bow });
    return this;
  }

  /** Measure `selector` (optionally containing `text`) at t0 into point `name`, then glide there. */
  moveTo(t0: number, t1: number, name: string, selector: string, opts: { text?: string; dx?: number; dy?: number; bow?: number; ax?: number; ay?: number } = {}): this {
    this.at(t0, `measure ${name}`, ({ ed, pts }) => {
      const r = ed.rect(selector, opts.text);
      if (!r) throw new Error(`[promo] ${name}: nothing matches ${selector} ${opts.text ?? ""}`);
      pts.set(name, { x: r.x + r.w * (opts.ax ?? 0.5) + (opts.dx ?? 0), y: r.y + r.h * (opts.ay ?? 0.5) + (opts.dy ?? 0) });
    });
    return this.glide(t0, t1, name, opts.bow ?? 0.1);
  }

  /** A real click at point `name`. */
  click(t: number, name: string, after?: (env: Env) => unknown): this {
    this.clicks.push(t);
    return this.at(t, `click ${name}`, async (env) => {
      const p = env.pts.get(name)!;
      env.ed.click(p.x, p.y);
      await env.ed.settle();
      await after?.(env);
    });
  }

  camera(t0: number, t1: number, to: string | Camera, e: Ease = ease.inOutCubic): this {
    this.camKeys.push({ t0, t1, to, ease: e });
    return this;
  }

  sort(): this {
    this.events.sort((a, b) => a.t - b.t);
    this.segs.sort((a, b) => a.t0 - b.t0);
    this.camKeys.sort((a, b) => a.t0 - b.t0);
    return this;
  }

  /** Cursor position (app coords) at time t, starting from `start`. */
  cursor(t: number, env: Env, start: Pt): Pt {
    let from = start;
    for (const s of this.segs) {
      const to = env.pts.get(s.to);
      if (!to || t < s.t0) break;
      if (t >= s.t1) {
        from = to;
        continue;
      }
      return arc(from, to, at(t, s.t0, s.t1, ease.smoother), s.bow);
    }
    return from;
  }

  cameraAt(t: number, env: Env, start: Camera = IDENTITY): Camera {
    let from = start;
    for (const k of this.camKeys) {
      if (t < k.t0) break;
      const to = typeof k.to === "string" ? env.cams.get(k.to) : k.to;
      if (!to) break;
      if (t >= k.t1) {
        from = to;
        continue;
      }
      return mixCamera(from, to, at(t, k.t0, k.t1, k.ease));
    }
    return from;
  }
}

export const screenOf = appToScreen;
