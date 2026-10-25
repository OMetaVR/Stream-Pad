// Pure helpers over graph macros: the linear "step list" view of a graph and edits on it.

import { newId } from "@/lib/model";
import type { GraphEdge, GraphNode, Macro } from "@/lib/types";
import { defaultParams, nodeDef, type Params } from "./catalog";

export type Graph = Extract<Macro, { type: "graph" }>;

const STEP_X = 260;
const ROW_Y = { press: 0, release: 220 };

export function emptyGraph(): Graph {
  return {
    type: "graph",
    nodes: [{ id: newId("n"), kind: "trigger.press", params: {}, position: { x: 0, y: ROW_Y.press } }],
    edges: [],
  };
}

export function newNode(kind: string, position = { x: 0, y: 0 }, params?: Params): GraphNode {
  return { id: newId("n"), kind, params: params ?? defaultParams(kind), position };
}

export function newEdge(source: string, sourcePort: string, target: string): GraphEdge {
  return { id: newId("e"), source, sourcePort, target };
}

export const isTrigger = (kind: string) => kind.startsWith("trigger.");

function outEdges(g: Graph, id: string) {
  return g.edges.filter((e) => e.source === id);
}

/**
 * The chain hanging off a trigger, if the graph is a plain sequence there: every node has one `out` edge
 * at most, nothing branches or loops, and nothing else points into the chain. `null` when it isn't linear.
 */
export function chainFrom(g: Graph, triggerKind: "trigger.press" | "trigger.release"): GraphNode[] | null {
  const trigger = g.nodes.find((n) => n.kind === triggerKind);
  if (!trigger) return [];
  const chain: GraphNode[] = [];
  const seen = new Set<string>([trigger.id]);
  let cur = trigger;
  for (;;) {
    const outs = outEdges(g, cur.id);
    if (outs.length > 1 || outs.some((e) => e.sourcePort !== "out")) return null;
    if (outs.length === 0) return chain;
    const next = g.nodes.find((n) => n.id === outs[0]!.target);
    if (!next) return chain;
    if (seen.has(next.id) || isTrigger(next.kind) || nodeDef(next.kind).outputs.some((o) => o.port !== "out")) return null;
    if (g.edges.filter((e) => e.target === next.id).length > 1) return null;
    seen.add(next.id);
    chain.push(next);
    cur = next;
  }
}

/** True when the whole graph can be shown as step lists (press chain + optional release chain). */
export function isLinear(g: Graph): boolean {
  const press = chainFrom(g, "trigger.press");
  const release = chainFrom(g, "trigger.release");
  if (!press || !release) return false;
  const used = new Set([...press, ...release].map((n) => n.id));
  return g.nodes.every((n) => isTrigger(n.kind) || used.has(n.id));
}

/** Rebuild a trigger's chain from an ordered node list (positions laid out left → right). */
export function setChain(g: Graph, triggerKind: "trigger.press" | "trigger.release", steps: GraphNode[]): Graph {
  let nodes = g.nodes.slice();
  let trigger = nodes.find((n) => n.kind === triggerKind);
  const row = triggerKind === "trigger.press" ? ROW_Y.press : ROW_Y.release;
  if (!trigger) {
    if (steps.length === 0) return g;
    trigger = { id: newId("n"), kind: triggerKind, params: {}, position: { x: 0, y: row } };
    nodes.push(trigger);
  }
  const old = chainFrom(g, triggerKind) ?? [];
  const keep = new Set(steps.map((s) => s.id));
  const dropped = new Set(old.filter((n) => !keep.has(n.id)).map((n) => n.id));
  nodes = nodes.filter((n) => !dropped.has(n.id) && !keep.has(n.id));
  const placed = steps.map((s, i) => ({ ...s, position: { x: (i + 1) * STEP_X, y: row } }));
  nodes.push(...placed);
  const chainIds = new Set([trigger.id, ...old.map((n) => n.id), ...placed.map((n) => n.id)]);
  const edges = g.edges.filter((e) => !chainIds.has(e.source) && !chainIds.has(e.target));
  let prev = trigger.id;
  for (const s of placed) {
    edges.push(newEdge(prev, "out", s.id));
    prev = s.id;
  }
  if (triggerKind === "trigger.release" && steps.length === 0) {
    nodes = nodes.filter((n) => n.id !== trigger!.id);
  }
  return { type: "graph", nodes, edges };
}

/** A graph with just `trigger.press → <kind>`. */
export function singleStepGraph(kind: string, params?: Params): Graph {
  return setChain(emptyGraph(), "trigger.press", [newNode(kind, undefined, params)]);
}

/** The first real action in the press chain (for default labels). */
export function firstAction(g: Graph): GraphNode | undefined {
  const press = g.nodes.find((n) => n.kind === "trigger.press");
  const e = press && g.edges.find((x) => x.source === press.id);
  return e ? g.nodes.find((n) => n.id === e.target) : undefined;
}

export function actionCount(m: Macro | null): number {
  if (!m) return 0;
  if (m.type === "script") return 1;
  return m.nodes.filter((n) => !isTrigger(n.kind)).length;
}
