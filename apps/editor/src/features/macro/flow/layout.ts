// "Tidy up": left-to-right layered layout with elkjs, respecting port order (yes above no, body above done).

import ELK from "elkjs/lib/elk.bundled.js";
import type { GraphNode } from "@/lib/types";
import { nodeDef } from "../catalog";
import type { Graph } from "../graph";
import { NODE_WIDTH } from "./FlowNode";

const elk = new ELK();

function height(n: GraphNode): number {
  const outs = nodeDef(n.kind).outputs.length;
  return outs > 1 ? 58 + outs * 20 : 56;
}

export async function tidy(g: Graph): Promise<Graph> {
  const res = await elk.layout({
    id: "root",
    layoutOptions: {
      "elk.algorithm": "layered",
      "elk.direction": "RIGHT",
      "elk.layered.spacing.nodeNodeBetweenLayers": "72",
      "elk.spacing.nodeNode": "28",
      "elk.layered.nodePlacement.strategy": "BRANDES_KOEPF",
      "elk.portConstraints": "FIXED_ORDER",
    },
    children: g.nodes.map((n) => ({
      id: n.id,
      width: n.kind.startsWith("trigger.") ? 172 : NODE_WIDTH,
      height: height(n),
      ports: nodeDef(n.kind).outputs.map((o, i) => ({ id: `${n.id}:${o.port}`, layoutOptions: { "elk.port.side": "EAST", "elk.port.index": String(i) } })),
    })),
    edges: g.edges.map((e) => ({ id: e.id, sources: [`${e.source}:${e.sourcePort}`], targets: [e.target] })),
  });
  const pos = new Map((res.children ?? []).map((c) => [c.id, { x: Math.round(c.x ?? 0), y: Math.round(c.y ?? 0) }]));
  return { ...g, nodes: g.nodes.map((n) => ({ ...n, position: pos.get(n.id) ?? n.position })) };
}
