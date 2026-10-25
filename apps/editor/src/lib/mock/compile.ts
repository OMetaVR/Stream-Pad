// Browser-mock stand-in for streampad_core::compile: turns a graph into plausible JS following the node
// catalog (docs/SPEC.md §3.3) so the Code tab and the simulated daemon have something real to show.
// The Rust compiler is authoritative; this one only needs to be faithful enough for development.

import type { CompileResult } from "../backend";
import type { Condition, GraphEdge, GraphNode, Macro } from "../types";

export class MockCompileError extends Error {
  constructor(
    message: string,
    readonly nodeId?: string,
  ) {
    super(message);
  }
}

const q = (v: unknown) => JSON.stringify(v ?? "");

/** `{{name}}` interpolation → template literal reading macro variables. */
function text(v: unknown): string {
  const s = String(v ?? "");
  if (!s.includes("{{")) return q(s);
  const body = s
    .replace(/[`\\]/g, (c) => `\\${c}`)
    .replace(/\$\{/g, "\\${")
    .replace(/\{\{\s*([\w.-]+)\s*\}\}/g, (_, name: string) => `\${vars.get(${q(name)}, "")}`);
  return `\`${body}\``;
}

function opts(o: Record<string, unknown>): string {
  const entries = Object.entries(o).filter(([, v]) => v !== undefined && v !== "");
  return entries.length ? `, { ${entries.map(([k, v]) => `${k}: ${JSON.stringify(v)}`).join(", ")} }` : "";
}

export function conditionJs(c: Condition | undefined): string {
  switch (c?.type) {
    case "activeApp":
      return `window.isActive(${q(c.match)})`;
    case "activeTitle":
      return `(window.active()?.title ?? "").toLowerCase().includes(${q(c.match.toLowerCase())})`;
    case "appRunning":
      return `window.isRunning(${q(c.match)})`;
    case "var": {
      const v = `vars.get(${q(c.name)})`;
      if (c.op === "falsy") return `!${v}`;
      if (c.op === "eq") return `String(${v}) === ${q(c.value)}`;
      if (c.op === "neq") return `String(${v}) !== ${q(c.value)}`;
      return `!!${v}`;
    }
    case "profile":
      return `profile.current().id === ${q(c.profileId)}`;
    case "time":
      return `__timeBetween(${q(c.from)}, ${q(c.to)})`;
    case "expr":
      return `(${c.code || "false"})`;
    default:
      return "false";
  }
}

/** One JS statement for an action node, or null for flow/trigger nodes and unknown kinds. */
export function actionJs(node: GraphNode): string | null {
  const p = node.params;
  switch (node.kind) {
    case "key.shortcut":
      return `press(${q(p.combo)});`;
    case "key.down":
      return `keyDown(${q(p.combo)});`;
    case "key.up":
      return `keyUp(${q(p.combo)});`;
    case "key.type":
      return `type(${text(p.text)}${opts({ delayMs: p.delayMs })});`;
    case "media":
      return `media(${q(p.action)});`;
    case "app.close":
      return `closeApp(${q(p.app)}${p.force ? ", { force: true }" : ""});`;
    case "app.launch":
      return `launch(${q(p.app)}${opts({ focusIfRunning: p.focusIfRunning ?? true, args: p.args })});`;
    case "open.url":
      return `openUrl(${text(p.url)});`;
    case "open.path":
      return `openPath(${text(p.path)});`;
    case "run.command":
      return `run(${text(p.command)}${opts({ wait: p.wait })});`;
    case "window.focus":
      return `window.focus(${q(p.match)}${opts({ by: p.by })});`;
    case "window.moveToOutput":
      return `window.moveToOutput(${q(p.target)});`;
    case "window.action":
      return `window.${String(p.action ?? "maximize")}();`;
    case "desktop.switch":
      return `desktop.switchTo(${JSON.stringify(p.target ?? "next")});`;
    case "desktop.moveWindow":
      return `desktop.moveWindow(${JSON.stringify(p.target ?? "next")}${opts({ follow: p.follow })});`;
    case "desktop.create":
      return "desktop.create();";
    case "desktop.remove":
      return "desktop.remove();";
    case "desktop.overview":
      return "desktop.overview();";
    case "kde.shortcut":
      return `kde.shortcut(${q(p.component)}, ${q(p.shortcut)});`;
    case "profile.switch":
      return `profile.set(${q(p.target)});`;
    case "macroMode.set":
      return p.value === "toggle" ? "macroMode.toggle();" : `macroMode.set(${p.value !== "off"});`;
    case "overlay.set":
      return `overlay.${String(p.value ?? "toggle")}();`;
    case "toast":
      return `toast(${text(p.text)}${opts({ durationMs: p.durationMs })});`;
    case "notify":
      return `notify(${text(p.title)}, ${text(p.body)});`;
    case "var.set":
      return `vars.set(${q(p.name)}, ${parseValue(p.value)});`;
    case "var.toggle":
      return `vars.toggle(${q(p.name)});`;
    case "clipboard.set":
      return `clipboard.set(${text(p.text)});`;
    case "sound.play":
      return `sound.play(${q(p.path)});`;
    case "delay":
      return `sleep(${Number(p.ms ?? 100)});`;
    case "script":
      return `{\n${indent(String(p.code ?? ""), 1)}\n}`;
    default:
      return null;
  }
}

function parseValue(v: unknown): string {
  const s = String(v ?? "");
  try {
    JSON.parse(s);
    return s;
  } catch {
    return q(s);
  }
}

function indent(code: string, depth: number): string {
  const pad = "  ".repeat(depth);
  return code
    .split("\n")
    .map((l) => (l ? pad + l : l))
    .join("\n");
}

const FLOW = new Set(["trigger.press", "trigger.release", "flow.if", "flow.repeat", "flow.stop"]);

export function compileGraph(nodes: GraphNode[], edges: GraphEdge[]): string {
  const byId = new Map(nodes.map((n) => [n.id, n]));
  const out = new Map(edges.map((e) => [`${e.source}:${e.sourcePort}`, e.target]));
  const presses = nodes.filter((n) => n.kind === "trigger.press");
  if (presses.length !== 1) throw new MockCompileError(presses.length ? "Only one press trigger is allowed" : "Add a press trigger");
  const releases = nodes.filter((n) => n.kind === "trigger.release");
  if (releases.length > 1) throw new MockCompileError("Only one release trigger is allowed", releases[1]!.id);
  for (const n of nodes) {
    if (!FLOW.has(n.kind) && actionJs(n) === null) throw new MockCompileError(`Unknown node kind "${n.kind}"`, n.id);
  }

  const emit = (from: string | undefined, depth: number, path: Set<string>): string[] => {
    const lines: string[] = [];
    for (let id = from; id; ) {
      if (path.has(id)) throw new MockCompileError("Wires form a loop — use a Repeat node instead", id);
      path.add(id);
      const node = byId.get(id)!;
      const pad = "  ".repeat(depth);
      if (node.kind === "flow.if") {
        lines.push(`${pad}if (${conditionJs(node.params.condition as Condition)}) {`);
        lines.push(...emit(out.get(`${id}:true`), depth + 1, new Set(path)));
        lines.push(`${pad}} else {`);
        lines.push(...emit(out.get(`${id}:false`), depth + 1, new Set(path)));
        lines.push(`${pad}}`);
        return lines;
      }
      if (node.kind === "flow.repeat") {
        lines.push(`${pad}for (let i = 0; i < ${Number(node.params.count ?? 2)}; i++) {`);
        lines.push(...emit(out.get(`${id}:body`), depth + 1, new Set(path)));
        lines.push(`${pad}}`);
        id = out.get(`${id}:done`);
        continue;
      }
      if (node.kind === "flow.stop") {
        lines.push(`${pad}return;`);
        return lines;
      }
      const js = actionJs(node);
      if (js) lines.push(indent(js, depth));
      id = out.get(`${id}:out`);
    }
    return lines;
  };

  const press = emit(out.get(`${presses[0]!.id}:out`), 0, new Set());
  const parts = [press.join("\n")];
  if (releases[0]) {
    parts.push(`function onRelease(event) {\n${emit(out.get(`${releases[0].id}:out`), 1, new Set()).join("\n")}\n}`);
  }
  return parts.filter(Boolean).join("\n\n") + "\n";
}

export function compileMock(m: Macro): CompileResult {
  if (m.type === "script") return { ok: true, code: m.source };
  try {
    return { ok: true, code: compileGraph(m.nodes, m.edges) };
  } catch (e) {
    if (e instanceof MockCompileError) return { ok: false, error: e.message, ...(e.nodeId ? { nodeId: e.nodeId } : {}) };
    throw e;
  }
}
