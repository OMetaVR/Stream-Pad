// A virtual clock for the promo's editor frame: setTimeout/setInterval, Date and performance.now all run on
// timeline time, so every debounce, flash and toast the real app schedules lands on the same frame in every
// capture. Importing this module installs it — import it before anything else in that realm.

type Fn = (...args: unknown[]) => void;
interface Timer {
  id: number;
  at: number;
  seq: number;
  fn: Fn | string;
  args: unknown[];
  every: number | null;
}

const RealDate = Date;
const EPOCH = RealDate.UTC(2026, 9, 24, 18, 30, 0);
let now = 0;
let seq = 0;
let nextId = 1;
const timers = new Map<number, Timer>();

function add(fn: Fn | string, ms: unknown, args: unknown[], repeat: boolean): number {
  const delay = Math.max(0, Number(ms) || 0);
  const id = nextId++;
  timers.set(id, { id, at: now + delay, seq: seq++, fn, args, every: repeat ? Math.max(1, delay) : null });
  return id;
}

const w = window as unknown as Record<string, unknown>;
w.setTimeout = (fn: Fn | string, ms?: number, ...args: unknown[]) => add(fn, ms, args, false);
w.setInterval = (fn: Fn | string, ms?: number, ...args: unknown[]) => add(fn, ms, args, true);
w.clearTimeout = (id?: number) => void (id != null && timers.delete(Number(id)));
w.clearInterval = w.clearTimeout;

class VirtualDate extends RealDate {
  constructor(...a: unknown[]) {
    if (a.length === 0) super(EPOCH + now);
    else super(...(a as [string]));
  }
  static override now() {
    return EPOCH + now;
  }
}
w.Date = VirtualDate;
Object.defineProperty(performance, "now", { configurable: true, value: () => now });

// Deterministic Math.random (mulberry32).
let seed = 0x5eed1234;
Math.random = () => {
  seed = (seed + 0x6d2b79f5) | 0;
  let x = Math.imul(seed ^ (seed >>> 15), 1 | seed);
  x = (x + Math.imul(x ^ (x >>> 7), 61 | x)) ^ x;
  return ((x ^ (x >>> 14)) >>> 0) / 4294967296;
};

let tickHook: (() => void) | null = null;
/** Called after every timer and settle round (used to pin CSS animations to the virtual clock). */
export function onTick(fn: () => void): void {
  tickHook = fn;
}

/** A real macrotask (MessageChannel), so promises, React work and layout can run. */
export function macrotask(): Promise<void> {
  return new Promise((resolve) => {
    const c = new MessageChannel();
    c.port1.onmessage = () => resolve();
    c.port2.postMessage(0);
  });
}

/** Wall-clock ms (for timeouts while waiting on real async work). */
export const realNow = () => RealDate.now();

export function clockNow(): number {
  return now;
}

/** Run every timer due up to `target` (ms) in order, yielding a real macrotask after each. */
export async function advanceTo(target: number): Promise<void> {
  for (let guard = 0; guard < 20000; guard++) {
    let next: Timer | null = null;
    for (const t of timers.values()) {
      if (t.at <= target && (!next || t.at < next.at || (t.at === next.at && t.seq < next.seq))) next = t;
    }
    if (!next) break;
    now = Math.max(now, next.at);
    if (next.every != null) {
      next.at += next.every;
      next.seq = seq++;
    } else {
      timers.delete(next.id);
    }
    try {
      if (typeof next.fn === "function") next.fn(...next.args);
    } catch (e) {
      console.error("[promo clock] timer failed", e);
    }
    await macrotask();
    tickHook?.();
  }
  now = Math.max(now, target);
  await macrotask();
  tickHook?.();
}

/** Let the page settle at the current time: run zero-delay timers and pending work a few rounds. */
export async function settle(rounds = 4): Promise<void> {
  for (let i = 0; i < rounds; i++) {
    await macrotask();
    await advanceTo(now);
  }
}
