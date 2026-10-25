// The browser mock's starting config: the five default profiles of docs/SPEC.md §7 as real graph macros,
// plus a few keys with cover images (public/mock/covers) so every rendering path can be exercised.

import { defaultSettings } from "../defaults";
import type { Binding, Condition, Config, GraphEdge, GraphNode, KeyId, Macro, Profile } from "../types";

interface Step {
  kind: string;
  params?: Record<string, unknown>;
}

const X_STEP = 260;
const BRANCH_Y = 110;

/** Builds graphs with stable, readable ids (`<key>_n1`, `<key>_e1`, …). */
class GraphBuilder {
  readonly nodes: GraphNode[] = [];
  readonly edges: GraphEdge[] = [];
  constructor(private readonly prefix: string) {}

  node(kind: string, params: Record<string, unknown>, x: number, y: number): string {
    const id = `${this.prefix}_n${this.nodes.length + 1}`;
    this.nodes.push({ id, kind, params, position: { x, y } });
    return id;
  }

  edge(source: string, target: string, sourcePort = "out"): void {
    this.edges.push({ id: `${this.prefix}_e${this.edges.length + 1}`, source, sourcePort, target });
  }

  /** Append `steps` after `from` (on `port`), left to right at height `y`. Returns the last node id. */
  chain(from: string, steps: Step[], x: number, y: number, port = "out"): string {
    let prev = from;
    let p = port;
    steps.forEach((s, i) => {
      const id = this.node(s.kind, s.params ?? {}, x + i * X_STEP, y);
      this.edge(prev, id, p);
      prev = id;
      p = "out";
    });
    return prev;
  }

  build(): Macro {
    return { type: "graph", nodes: this.nodes, edges: this.edges };
  }
}

function steps(prefix: string, ...list: Step[]): Macro {
  const g = new GraphBuilder(prefix);
  g.chain(g.node("trigger.press", {}, 0, 0), list, X_STEP, 0);
  return g.build();
}

function branch(prefix: string, condition: Condition, yes: Step[], no: Step[], before: Step[] = []): Macro {
  const g = new GraphBuilder(prefix);
  const start = g.node("trigger.press", {}, 0, 0);
  const last = g.chain(start, before, X_STEP, 0);
  const x = X_STEP * (before.length + 1);
  const cond = g.node("flow.if", { condition }, x, 0);
  g.edge(last, cond);
  g.chain(cond, yes, x + X_STEP, -BRANCH_Y, "true");
  g.chain(cond, no, x + X_STEP, BRANCH_Y, "false");
  return g.build();
}

function key(label: string, icon: string, macro: Macro, extra: Partial<Binding> = {}): Binding {
  return { label, icon, cover: null, coverCrop: null, color: null, macro, repeat: false, concurrency: "ignore", ...extra };
}

const lucide = (name: string) => `lucide:${name}`;
const BROWSERS = "firefox,chromium,chrome,opera,brave,vivaldi,edge,zen,librewolf";

function audio(): Profile {
  const media = (action: string): Step => ({ kind: "media", params: { action } });
  const keys: Partial<Record<KeyId, Binding>> = {
    KP5: key("Play / Pause", lucide("play"), steps("kp5", media("playPause"))),
    KP4: key(
      "Back",
      lucide("skip-back"),
      branch("kp4", { type: "activeApp", match: BROWSERS }, [{ kind: "key.shortcut", params: { combo: "alt+left" } }], [media("prev")]),
    ),
    KP6: key(
      "Forward",
      lucide("skip-forward"),
      branch("kp6", { type: "activeApp", match: BROWSERS }, [{ kind: "key.shortcut", params: { combo: "alt+right" } }], [media("next")]),
    ),
    KP8: key("Volume up", lucide("volume-2"), steps("kp8", media("volumeUp")), { repeat: true }),
    KP2: key("Volume down", lucide("volume-1"), steps("kp2", media("volumeDown")), { repeat: true }),
    KP0: key("Mute", lucide("volume-x"), steps("kp0", media("mute"))),
    KP3: key(
      "Game mode",
      lucide("gamepad-2"),
      branch(
        "kp3",
        { type: "var", name: "gameMode", op: "truthy" },
        [{ kind: "toast", params: { text: "Game mode: ON", durationMs: 1600 } }],
        [{ kind: "toast", params: { text: "Game mode: OFF", durationMs: 1600 } }],
        [{ kind: "var.toggle", params: { name: "gameMode" } }],
      ),
    ),
    KP9: key("Spotify", lucide("music"), steps("kp9", { kind: "app.launch", params: { app: "spotify", focusIfRunning: true } }), {
      cover: "covers/synthwave.png",
    }),
  };
  return { id: "p_a0d10001", name: "Audio", color: "#8b7cf6", icon: lucide("audio-lines"), keys };
}

function apps(): Profile {
  const launch = (prefix: string, label: string, app: string): Binding =>
    key(label, `/mock/apps/${app}.svg`, steps(prefix, { kind: "app.launch", params: { app, focusIfRunning: true } }));
  return {
    id: "p_a995e002",
    name: "Apps",
    color: "#4aa8ff",
    icon: lucide("layout-grid"),
    keys: {
      KP1: launch("kp1", "Discord", "discord"),
      KP2: launch("kp2", "Firefox", "firefox"),
      KP3: launch("kp3", "Steam", "steam"),
      KP4: launch("kp4", "Spotify", "spotify"),
      KP5: launch("kp5", "System Monitor", "org.kde.plasma-systemmonitor"),
      KP6: launch("kp6", "Dolphin", "org.kde.dolphin"),
      KP7: launch("kp7", "Konsole", "org.kde.konsole"),
      KP8: launch("kp8", "Volume Control", "org.pulseaudio.pavucontrol"),
      KP9: key(
        "Go live",
        lucide("radio-tower"),
        steps(
          "kp9",
          { kind: "app.launch", params: { app: "com.obsproject.Studio", focusIfRunning: true, args: "--startstreaming" } },
          { kind: "toast", params: { text: "Going live…", durationMs: 1600 } },
        ),
        { cover: "covers/live.gif" },
      ),
    },
  };
}

function web(): Profile {
  const site = (prefix: string, label: string, icon: string, url: string) =>
    key(label, lucide(icon), steps(prefix, { kind: "open.url", params: { url } }));
  return {
    id: "p_3eb00003",
    name: "Web",
    color: "#3fcf8e",
    icon: lucide("globe"),
    keys: {
      KP1: site("kp1", "YouTube", "monitor-play", "https://www.youtube.com"),
      KP2: site("kp2", "Google", "search", "https://www.google.com"),
      KP4: site("kp4", "GitHub", "git-branch", "https://github.com"),
      KP5: site("kp5", "Discord", "messages-square", "https://discord.com/app"),
      KP7: site("kp7", "Twitch", "tv", "https://www.twitch.tv"),
      KP8: site("kp8", "Kick", "zap", "https://kick.com"),
      KP0: { ...site("kp0", "ChatGPT", "sparkles", "https://chatgpt.com"), cover: "covers/aurora.webp" },
    },
  };
}

function windows(): Profile {
  const move = (prefix: string, label: string, icon: string, target: string) =>
    key(label, lucide(icon), steps(prefix, { kind: "window.moveToOutput", params: { target } }));
  const act = (prefix: string, label: string, icon: string, action: string) =>
    key(label, lucide(icon), steps(prefix, { kind: "window.action", params: { action } }));
  return {
    id: "p_712d0004",
    name: "Windows",
    color: "#f5a524",
    icon: lucide("app-window"),
    keys: {
      KP4: move("kp4", "Monitor left", "arrow-left-to-line", "left"),
      KP6: move("kp6", "Monitor right", "arrow-right-to-line", "right"),
      KP8: move("kp8", "Monitor up", "arrow-up-to-line", "up"),
      KP2: move("kp2", "Monitor down", "arrow-down-to-line", "down"),
      KP5: act("kp5", "Maximize", "maximize-2", "maximize"),
      KP7: act("kp7", "Minimize", "minimize-2", "minimize"),
      KP9: act("kp9", "Keep above", "pin", "keepAbove"),
      KP1: act("kp1", "Center", "locate", "center"),
      KP3: act("kp3", "Fullscreen", "fullscreen", "fullscreen"),
    },
  };
}

function desktops(): Profile {
  return {
    id: "p_de5c0005",
    name: "Desktops",
    color: "#ff7a59",
    icon: lucide("layers"),
    keys: {
      KP4: key("Desktop left", lucide("arrow-left"), steps("kp4", { kind: "desktop.switch", params: { target: "left" } })),
      KP6: key("Desktop right", lucide("arrow-right"), steps("kp6", { kind: "desktop.switch", params: { target: "right" } })),
      KP1: key("Send left", lucide("chevrons-left"), steps("kp1", { kind: "desktop.moveWindow", params: { target: "prev", follow: true } })),
      KP3: key("Send right", lucide("chevrons-right"), steps("kp3", { kind: "desktop.moveWindow", params: { target: "next", follow: true } })),
      KP8: key("New desktop", lucide("square-plus"), steps("kp8", { kind: "desktop.create" })),
      KP2: key("Remove desktop", lucide("square-minus"), steps("kp2", { kind: "desktop.remove" })),
      KP5: key("Overview", lucide("layout-dashboard"), steps("kp5", { kind: "desktop.overview" })),
    },
  };
}

export function sampleConfig(): Config {
  return {
    version: 1,
    settings: defaultSettings(),
    profiles: [audio(), apps(), web(), windows(), desktops()],
    savedIcons: [],
    library: "// Helpers shared by every macro.\nfunction notifyDone(what) {\n  toast(`${what} done`, { kind: \"success\" });\n}\n",
  };
}
