// Simulated streampadd for the browser mock: answers the IPC methods of docs/SPEC.md §4.1, keeps runtime
// state (macro mode, profile, overlay pin/peek/preview, learn, vars) and "runs" macros by walking graphs
// with small delays, emitting run / trace / log / toast events like the real daemon.

import { GLOBAL_ACTION_INFO, globalOf } from "../model";
import { KEY_IDS, KEY_INFO } from "../types";
import type {
  Condition,
  Config,
  DaemonEvent,
  DaemonState,
  GlobalAction,
  GraphNode,
  KeyId,
  Macro,
  RenderKey,
} from "../types";
import type { DaemonChannel } from "../daemon-channel";
import { actionJs, compileMock } from "./compile";
import { MOCK_DEVICES, MOCK_OUTPUTS, MOCK_WINDOWS } from "./fixtures";

const VERSION = "0.1.0-mock";
const STEP_MS = 240;
const MAX_DELAY_MS = 2500;
const LEARN_TIMEOUT_MS = 30_000;

class Cancelled extends Error {}
class Stopped extends Error {}

interface Run {
  id: number;
  key: KeyId | null;
  abort: AbortController;
}

function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) return reject(new Cancelled());
    const t = setTimeout(resolve, ms);
    signal.addEventListener("abort", () => (clearTimeout(t), reject(new Cancelled())), { once: true });
  });
}

const tick = () => new Promise((r) => setTimeout(r, 25 + Math.random() * 40));

export class MockDaemon {
  running = false;
  private macroMode = false;
  private profileIndex = 0;
  private pinned = false;
  private peeking = false;
  private preview = false;
  private learn = false;
  private vars: Record<string, unknown> = {};
  private runs = new Map<number, Run>();
  private nextRunId = 1;
  private nextToastId = 1;
  private timers = { peek: 0, preview: 0, learn: 0, start: 0 };
  private holds = new Map<KeyId, { timer: number; fired: boolean }>();

  constructor(
    private readonly channel: DaemonChannel,
    private readonly config: () => Config,
    private readonly iconUrl: (ref: string) => string | null,
  ) {}

  // ---- lifecycle ---------------------------------------------------------------------------------------

  start(delayMs = 450): void {
    if (this.running) return;
    clearTimeout(this.timers.start);
    this.channel.setStatus({ kind: "connecting" });
    this.timers.start = window.setTimeout(() => {
      this.running = true;
      this.channel.setStatus({ kind: "connected", state: this.state() });
    }, delayMs);
  }

  stop(reason = "The daemon closed the connection"): void {
    clearTimeout(this.timers.start);
    for (const run of this.runs.values()) run.abort.abort();
    this.running = false;
    this.learn = false;
    this.channel.setStatus({ kind: "stopped", reason });
  }

  /** The editor saved config.json — the real daemon hot-reloads it. */
  configChanged(): void {
    if (!this.running) return;
    this.profileIndex = Math.min(this.profileIndex, this.config().profiles.length - 1);
    this.emitState();
  }

  // ---- state -------------------------------------------------------------------------------------------

  private state(): DaemonState {
    const cfg = this.config();
    const profile = cfg.profiles[this.profileIndex] ?? cfg.profiles[0]!;
    const keys = {} as Record<KeyId, RenderKey>;
    for (const k of KEY_IDS) {
      const g = globalOf(cfg.settings, k);
      const b = profile.keys[k];
      const icon = g ? `lucide:${GLOBAL_ACTION_INFO[g.tap].icon}` : b?.icon;
      keys[k] = {
        label: g ? GLOBAL_ACTION_INFO[g.tap].label : (b?.label ?? ""),
        icon: icon ? this.iconUrl(icon) : null,
        iconTint: !!icon?.startsWith("lucide:"),
        cover: !g && b?.cover ? this.iconUrl(b.cover) : null,
        coverCrop: !g && b?.cover ? (b.coverCrop ?? null) : null,
        color: g ? null : (b?.color ?? null),
        kind: g ? "global" : b?.macro ? "macro" : "empty",
        global: g?.tap ?? null,
        legend: KEY_INFO[k].legend,
      };
    }
    const overlay = cfg.settings.overlay;
    return {
      version: VERSION,
      macroMode: this.macroMode,
      profileIndex: this.profileIndex,
      profiles: cfg.profiles.map((p) => ({ id: p.id, name: p.name, color: p.color, icon: p.icon && this.iconUrl(p.icon) })),
      profile: { id: profile.id, name: profile.name, color: profile.color, icon: profile.icon && this.iconUrl(profile.icon), keys },
      overlay: {
        visible: this.preview || (this.macroMode && (this.pinned || this.peeking)),
        pinned: this.pinned,
        preview: this.preview,
        output: overlay.monitor ?? "DP-2",
        settings: overlay,
        accent: profile.color ?? overlay.accent,
      },
      learn: this.learn,
      devices: MOCK_DEVICES,
      integrations: { kwin: true, layerShell: true },
      configError: null,
    };
  }

  private emit(e: DaemonEvent): void {
    if (this.running) this.channel.pushEvent(e);
  }

  private emitState(): void {
    this.emit({ event: "state", data: this.state() });
  }

  private toast(text: string, kind: "info" | "success" | "error" = "info", durationMs = 1600): void {
    this.emit({ event: "toast", data: { id: this.nextToastId++, text, kind, durationMs } });
  }

  private peek(): void {
    const ms = this.config().settings.overlay.peekMs;
    this.peeking = true;
    clearTimeout(this.timers.peek);
    this.timers.peek = window.setTimeout(() => {
      this.peeking = false;
      this.emitState();
    }, ms);
  }

  private setMacroMode(on: boolean): void {
    if (this.macroMode === on) return;
    this.macroMode = on;
    if (on && this.config().settings.overlay.peekOnModeChange) this.peek();
    this.emitState();
  }

  private setProfile(index: number, direction: 1 | -1): void {
    const n = this.config().profiles.length;
    const next = ((index % n) + n) % n;
    if (next === this.profileIndex) return;
    this.profileIndex = next;
    if (this.macroMode && this.config().settings.overlay.peekOnProfileChange) this.peek();
    this.emit({ event: "profile", data: { index: next, direction } });
    this.emitState();
  }

  private globalAction(action: GlobalAction): void {
    switch (action) {
      case "toggleOverlay":
        this.pinned = !this.pinned;
        break;
      case "showOverlay":
        this.pinned = true;
        break;
      case "hideOverlay":
        this.pinned = false;
        this.peeking = false;
        break;
      case "nextProfile":
        return this.setProfile(this.profileIndex + 1, 1);
      case "prevProfile":
        return this.setProfile(this.profileIndex - 1, -1);
      case "firstProfile":
        return this.setProfile(0, -1);
      case "toggleMacroMode":
        return this.setMacroMode(!this.macroMode);
      case "openEditor":
        return this.toast("The editor is already open");
      case "none":
        return;
    }
    this.emitState();
  }

  // ---- simulated hardware ------------------------------------------------------------------------------

  /** A captured pad key event, exactly like `simulateKey`. "Toggle" flips macro mode. */
  pressKey(key: KeyId | "Toggle", pressed: boolean): void {
    if (!this.running) return;
    if (key === "Toggle") {
      if (pressed) this.setMacroMode(!this.macroMode);
      return;
    }
    if (!this.macroMode && !this.learn) return;
    const cfg = this.config();
    const g = globalOf(cfg.settings, key);
    const binding = cfg.profiles[this.profileIndex]?.keys[key];
    const kind = this.learn ? "learn" : g ? "global" : binding?.macro ? "macro" : "empty";
    this.emit({ event: "key", data: { key, pressed, kind } });
    if (this.learn) {
      if (pressed) this.emit({ event: "learn", data: { key } });
      return;
    }
    if (g) return this.globalKey(key, g.tap, g.hold, pressed);
    if (pressed && binding?.macro) {
      const active = [...this.runs.values()].filter((r) => r.key === key);
      if (active.length && binding.concurrency === "ignore") return;
      if (binding.concurrency === "restart") active.forEach((r) => r.abort.abort());
      this.startRun(binding.macro, key);
    }
  }

  private globalKey(key: KeyId, tap: GlobalAction, hold: GlobalAction | null, pressed: boolean): void {
    if (!hold) {
      if (pressed) this.globalAction(tap);
      return;
    }
    if (pressed) {
      const state = { timer: 0, fired: false };
      state.timer = window.setTimeout(() => {
        state.fired = true;
        this.globalAction(hold);
      }, this.config().settings.holdMs);
      this.holds.set(key, state);
      return;
    }
    const state = this.holds.get(key);
    this.holds.delete(key);
    if (!state) return;
    clearTimeout(state.timer);
    if (!state.fired) this.globalAction(tap);
  }

  // ---- macro runs --------------------------------------------------------------------------------------

  private startRun(macro: Macro, key: KeyId | null): number {
    const run: Run = { id: this.nextRunId++, key, abort: new AbortController() };
    this.runs.set(run.id, run);
    this.emit({ event: "run", data: { runId: run.id, key, status: "started" } });
    void this.execute(run, macro);
    return run.id;
  }

  private async execute(run: Run, macro: Macro): Promise<void> {
    const done = (status: "finished" | "failed" | "cancelled", error?: string) => {
      this.runs.delete(run.id);
      this.emit({ event: "run", data: { runId: run.id, key: run.key, status, ...(error ? { error } : {}) } });
    };
    try {
      if (macro.type === "script") await this.runScript(run, macro.source);
      else await this.runGraph(run, macro);
      done("finished");
    } catch (e) {
      if (e instanceof Cancelled) return done("cancelled");
      if (e instanceof Stopped) return done("finished");
      const message = e instanceof Error ? e.message : String(e);
      this.log(run, "error", message);
      if (this.config().settings.toastOnError) this.toast(message, "error");
      done("failed", message);
    }
  }

  private log(run: Run, level: "info" | "warn" | "error", message: string): void {
    this.emit({ event: "log", data: { runId: run.id, level, message } });
  }

  private async runScript(run: Run, source: string): Promise<void> {
    // Syntax check only; the mock never executes user code.
    new Function(this.config().library + "\n" + source);
    await sleep(STEP_MS, run.abort.signal);
    for (const m of source.matchAll(/\blog\(\s*(["'`])((?:(?!\1).)*)\1/g)) {
      this.log(run, "info", m[2]!);
      await sleep(120, run.abort.signal);
    }
  }

  private async runGraph(run: Run, macro: Extract<Macro, { type: "graph" }>): Promise<void> {
    const compiled = compileMock(macro);
    if (!compiled.ok) throw new Error(compiled.error);
    const byId = new Map(macro.nodes.map((n) => [n.id, n]));
    const next = new Map(macro.edges.map((e) => [`${e.source}:${e.sourcePort}`, e.target]));
    const trigger = macro.nodes.find((n) => n.kind === "trigger.press")!;

    const walk = async (start: string | undefined): Promise<void> => {
      for (let id = start; id; ) {
        const node = byId.get(id);
        if (!node) return;
        this.emit({ event: "trace", data: { runId: run.id, nodeId: id } });
        await sleep(node.kind === "delay" ? Math.min(Number(node.params.ms ?? 100), MAX_DELAY_MS) : STEP_MS, run.abort.signal);
        if (node.kind === "flow.stop") throw new Stopped();
        if (node.kind === "flow.repeat") {
          const count = Math.min(Math.max(0, Number(node.params.count ?? 2)), 20);
          for (let i = 0; i < count; i++) await walk(next.get(`${id}:body`));
          id = next.get(`${id}:done`);
          continue;
        }
        if (node.kind === "flow.if") {
          id = next.get(`${id}:${this.evaluate(node.params.condition as Condition) ? "true" : "false"}`);
          continue;
        }
        this.perform(run, node);
        id = next.get(`${id}:out`);
      }
    };
    await walk(trigger.id);
  }

  private evaluate(c: Condition | undefined): boolean {
    const active = MOCK_WINDOWS.find((w) => w.active);
    const matches = (list: string, w: (typeof MOCK_WINDOWS)[number]) =>
      list
        .split(",")
        .map((s) => s.trim().toLowerCase())
        .filter(Boolean)
        .some((m) => [w.appId, w.resourceClass, w.desktopFile].some((f) => f.toLowerCase().includes(m)));
    switch (c?.type) {
      case "activeApp":
        return !!active && matches(c.match, active);
      case "appRunning":
        return MOCK_WINDOWS.some((w) => matches(c.match, w));
      case "activeTitle":
        return !!active && active.title.toLowerCase().includes(c.match.toLowerCase());
      case "var": {
        const v = this.vars[c.name];
        if (c.op === "falsy") return !v;
        if (c.op === "eq") return String(v ?? "") === (c.value ?? "");
        if (c.op === "neq") return String(v ?? "") !== (c.value ?? "");
        return !!v;
      }
      case "profile":
        return this.config().profiles[this.profileIndex]?.id === c.profileId;
      default:
        return true;
    }
  }

  /** Log what the real daemon would do, and apply the effects the mock can model. */
  private perform(run: Run, node: GraphNode): void {
    const p = node.params;
    if (node.kind === "trigger.press" || node.kind === "trigger.release") return;
    this.log(run, "info", actionJs(node) ?? node.kind);
    const interpolate = (s: unknown) => String(s ?? "").replace(/\{\{\s*([\w.-]+)\s*\}\}/g, (_, n: string) => String(this.vars[n] ?? ""));
    switch (node.kind) {
      case "var.toggle":
        this.vars[String(p.name)] = !this.vars[String(p.name)];
        break;
      case "var.set":
        this.vars[String(p.name)] = p.value;
        break;
      case "toast":
        this.toast(interpolate(p.text), "info", Number(p.durationMs ?? 1600));
        break;
      case "profile.switch":
        if (p.target === "next") this.setProfile(this.profileIndex + 1, 1);
        else if (p.target === "prev") this.setProfile(this.profileIndex - 1, -1);
        else {
          const i = this.config().profiles.findIndex((x) => x.id === p.target);
          if (i >= 0) this.setProfile(i, i > this.profileIndex ? 1 : -1);
        }
        break;
      case "macroMode.set":
        this.setMacroMode(p.value === "toggle" ? !this.macroMode : p.value !== "off");
        break;
      case "overlay.set":
        if (p.value === "peek") this.peek();
        else this.pinned = p.value === "toggle" ? !this.pinned : p.value === "show";
        this.emitState();
        break;
    }
  }

  // ---- IPC ---------------------------------------------------------------------------------------------

  async request(method: string, params: unknown): Promise<unknown> {
    if (!this.running) throw new Error("The StreamPad daemon is not running");
    await tick();
    const p = (params ?? {}) as Record<string, unknown>;
    switch (method) {
      case "hello":
        return { version: VERSION, state: this.state() };
      case "getState":
        return this.state();
      case "setMacroMode":
        this.setMacroMode(p.toggle ? !this.macroMode : !!p.on);
        return { macroMode: this.macroMode };
      case "setProfile": {
        const profiles = this.config().profiles;
        if (typeof p.delta === "number") this.setProfile(this.profileIndex + p.delta, p.delta >= 0 ? 1 : -1);
        else if (typeof p.index === "number") this.setProfile(p.index, p.index >= this.profileIndex ? 1 : -1);
        else if (typeof p.id === "string") {
          const i = profiles.findIndex((x) => x.id === p.id);
          if (i < 0) throw new Error(`No profile with id ${p.id}`);
          this.setProfile(i, i >= this.profileIndex ? 1 : -1);
        }
        return { profileIndex: this.profileIndex };
      }
      case "overlay":
        return this.overlayRequest(String(p.action ?? "toggle"), !!p.preview, Number(p.durationMs ?? 0));
      case "runBinding": {
        const profile = this.config().profiles.find((x) => x.id === p.profileId);
        const binding = profile?.keys[p.key as KeyId];
        if (!binding?.macro) throw new Error("That key has no macro");
        return { runId: this.startRun(binding.macro, p.key as KeyId) };
      }
      case "runMacro":
        return { runId: this.startRun(p.macro as Macro, (p.key as KeyId | undefined) ?? null) };
      case "cancelRun":
        this.runs.get(Number(p.runId))?.abort.abort();
        return {};
      case "compileMacro": {
        const r = compileMock(p.macro as Macro);
        if (!r.ok) throw new Error(r.error);
        return { code: r.code };
      }
      case "setLearn":
        this.learn = !!p.on;
        clearTimeout(this.timers.learn);
        if (this.learn) {
          this.timers.learn = window.setTimeout(() => {
            this.learn = false;
            this.emitState();
          }, LEARN_TIMEOUT_MS);
        }
        this.emitState();
        return {};
      case "listDevices":
        return MOCK_DEVICES;
      case "listOutputs":
        return MOCK_OUTPUTS;
      case "listWindows":
        return MOCK_WINDOWS;
      case "reload":
        this.emitState();
        return {};
      case "simulateKey":
        this.pressKey(p.key as KeyId | "Toggle", p.pressed !== false);
        return {};
      case "getVars":
        return { ...this.vars };
      case "setVar":
        this.vars[String(p.name)] = p.value;
        return {};
      case "openEditor":
        return {};
      case "quit":
        setTimeout(() => this.stop("The daemon was asked to quit"), 0);
        return {};
      default:
        throw new Error(`Unknown method "${method}"`);
    }
  }

  private overlayRequest(action: string, preview: boolean, durationMs: number): { visible: boolean } {
    if (preview) {
      clearTimeout(this.timers.preview);
      this.preview = action === "hide" ? false : action === "toggle" ? !this.preview : true;
      if (this.preview && action === "peek") {
        this.timers.preview = window.setTimeout(() => {
          this.preview = false;
          this.emitState();
        }, durationMs || this.config().settings.overlay.peekMs);
      }
    } else if (action === "peek") {
      this.peek();
    } else {
      this.pinned = action === "toggle" ? !this.pinned : action === "show";
    }
    this.emitState();
    return { visible: this.state().overlay.visible };
  }
}
