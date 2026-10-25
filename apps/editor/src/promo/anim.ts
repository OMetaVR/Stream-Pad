// Easing + interpolation helpers for the promo timeline. Everything is a pure function of time (seconds).

export type Ease = (x: number) => number;

export const clamp01 = (x: number) => (x < 0 ? 0 : x > 1 ? 1 : x);
export const clamp = (x: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, x));
export const lerp = (a: number, b: number, k: number) => a + (b - a) * k;

export const ease = {
  linear: ((x) => x) as Ease,
  inQuad: ((x) => x * x) as Ease,
  outQuad: ((x) => 1 - (1 - x) * (1 - x)) as Ease,
  inCubic: ((x) => x * x * x) as Ease,
  outCubic: ((x) => 1 - Math.pow(1 - x, 3)) as Ease,
  inOutCubic: ((x) => (x < 0.5 ? 4 * x * x * x : 1 - Math.pow(-2 * x + 2, 3) / 2)) as Ease,
  inQuint: ((x) => x ** 5) as Ease,
  outQuint: ((x) => 1 - Math.pow(1 - x, 5)) as Ease,
  inOutQuint: ((x) => (x < 0.5 ? 16 * x ** 5 : 1 - Math.pow(-2 * x + 2, 5) / 2)) as Ease,
  outExpo: ((x) => (x >= 1 ? 1 : 1 - Math.pow(2, -10 * x))) as Ease,
  inOutSine: ((x) => -(Math.cos(Math.PI * x) - 1) / 2) as Ease,
  /** Minimum-jerk profile — how a hand moves a mouse. */
  smoother: ((x) => x * x * x * (x * (x * 6 - 15) + 10)) as Ease,
  /** Back-out with a configurable overshoot (fraction of the distance, e.g. 0.04 = 4 %). */
  outBack(overshoot = 0.04): Ease {
    // Solve s for the requested peak overshoot of the standard easeOutBack curve (peak ≈ 0.1 at s = 1.70158).
    const s = backS(overshoot);
    return (x) => 1 + (s + 1) * Math.pow(x - 1, 3) + s * Math.pow(x - 1, 2);
  },
};

const backCache = new Map<number, number>();
function backS(overshoot: number): number {
  const hit = backCache.get(overshoot);
  if (hit !== undefined) return hit;
  const peak = (s: number) => {
    let m = 0;
    for (let i = 0; i <= 200; i++) {
      const x = i / 200;
      m = Math.max(m, 1 + (s + 1) * Math.pow(x - 1, 3) + s * Math.pow(x - 1, 2));
    }
    return m - 1;
  };
  let lo = 0;
  let hi = 4;
  for (let i = 0; i < 40; i++) {
    const mid = (lo + hi) / 2;
    if (peak(mid) < overshoot) lo = mid;
    else hi = mid;
  }
  backCache.set(overshoot, lo);
  return lo;
}

/** Eased progress of `t` through [t0, t1] (0 before, 1 after). */
export function at(t: number, t0: number, t1: number, e: Ease = ease.linear): number {
  if (t1 <= t0) return t >= t1 ? 1 : 0;
  return e(clamp01((t - t0) / (t1 - t0)));
}

/** Fade in over [a, b], hold, fade out over [c, d]. */
export function window4(t: number, a: number, b: number, c: number, d: number, eIn: Ease = ease.outCubic, eOut: Ease = ease.inCubic): number {
  if (t < a || t > d) return 0;
  if (t < b) return at(t, a, b, eIn);
  if (t <= c) return 1;
  return 1 - at(t, c, d, eOut);
}

/** Piecewise tween through keyframes `[time, value, easeIntoThisKey?]`. */
export function keys(t: number, frames: [number, number, Ease?][]): number {
  if (t <= frames[0]![0]) return frames[0]![1];
  for (let i = 1; i < frames.length; i++) {
    const [t1, v1, e] = frames[i]!;
    const [t0, v0] = frames[i - 1]!;
    if (t <= t1) return lerp(v0, v1, at(t, t0, t1, e ?? ease.inOutCubic));
  }
  return frames[frames.length - 1]![1];
}

export interface Pt {
  x: number;
  y: number;
}

export const mixPt = (a: Pt, b: Pt, k: number): Pt => ({ x: lerp(a.x, b.x, k), y: lerp(a.y, b.y, k) });

/** Quadratic Bézier from a to b bowed sideways by `bow` × distance (positive = clockwise). */
export function arc(a: Pt, b: Pt, k: number, bow = 0.12): Pt {
  const dx = b.x - a.x;
  const dy = b.y - a.y;
  const c = { x: (a.x + b.x) / 2 - dy * bow, y: (a.y + b.y) / 2 + dx * bow };
  const u = 1 - k;
  return { x: u * u * a.x + 2 * u * k * c.x + k * k * b.x, y: u * u * a.y + 2 * u * k * c.y + k * k * b.y };
}
