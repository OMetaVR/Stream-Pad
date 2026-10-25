// Custom React Flow node: a compact card with a category stripe, icon, title, one-line summary and
// labelled exec ports (input left, outputs right).

import { Handle, Position, type Node, type NodeProps } from "@xyflow/react";
import { AlertCircle } from "lucide-react";
import { memo } from "react";
import { cn } from "@/lib/cn";
import type { GraphNode } from "@/lib/types";
import { RefIcon } from "@/ui";
import { categoryColor, nodeDef, type NodeContext } from "../catalog";
import { isTrigger } from "../graph";

export interface FlowNodeData extends Record<string, unknown> {
  node: GraphNode;
  ctx: NodeContext;
  error: string | null;
  traced: boolean;
  unreachable: boolean;
}

export type FlowNodeType = Node<FlowNodeData, "step">;

export const NODE_WIDTH = 220;

export const FlowNode = memo(function FlowNode({ data, selected }: NodeProps<FlowNodeType>) {
  const { node, ctx, error, traced, unreachable } = data;
  const def = nodeDef(node.kind);
  const color = categoryColor(def.category);
  const trigger = isTrigger(node.kind);
  const outs = def.outputs;
  const multi = outs.length > 1;

  return (
    <div
      className={cn(
        "relative rounded-md border bg-raised shadow-raised transition-[border-color,box-shadow,opacity] duration-150",
        selected ? "border-accent-line ring-2 ring-accent/30" : "border-line",
        error && !selected && "border-warning/60",
        traced && "ring-4 ring-accent/50",
        unreachable && "opacity-45",
        trigger && "rounded-full",
      )}
      style={{ width: trigger ? 172 : NODE_WIDTH }}
      title={unreachable ? "Not connected to a trigger — this step never runs" : undefined}
    >
      {!trigger && <div className="absolute inset-y-2 left-0 w-[3px] rounded-r-full" style={{ background: color }} />}
      <div className={cn("flex items-center gap-2.5", trigger ? "px-3 py-2" : "px-3 py-2.5")}>
        <span
          className={cn("flex shrink-0 items-center justify-center", trigger ? "size-6 rounded-full" : "size-7 rounded-sm")}
          style={{ background: `color-mix(in srgb, ${color} 18%, transparent)`, color }}
        >
          <RefIcon iconRef={`lucide:${def.icon}`} size={trigger ? 13 : 15} />
        </span>
        <div className="min-w-0 flex-1">
          <div className="truncate text-base font-medium text-fg">{def.title}</div>
          {!trigger && <div className="truncate text-xs text-muted">{def.summary(node.params, ctx) || " "}</div>}
        </div>
        {error && <AlertCircle size={14} className="shrink-0 text-warning" aria-label={error} />}
      </div>
      {multi && (
        <div className="flex flex-col gap-1 border-t border-line-subtle py-1.5 pr-3">
          {outs.map((o) => (
            <div key={o.port} className="relative text-right text-xs text-muted">
              {o.label}
              <Handle type="source" id={o.port} position={Position.Right} className="sp-handle !-right-[17px]" />
            </div>
          ))}
        </div>
      )}
      {!trigger && <Handle type="target" id="in" position={Position.Left} className="sp-handle" />}
      {!multi && outs.length === 1 && <Handle type="source" id={outs[0]!.port} position={Position.Right} className="sp-handle" />}
      {traced && <span className="pointer-events-none absolute -inset-px animate-ping rounded-md border-2 border-accent opacity-40" />}
    </div>
  );
});
