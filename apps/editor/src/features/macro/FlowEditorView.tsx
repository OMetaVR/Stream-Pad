// Full-window node editor for one key's macro (docs/SPEC.md §9): palette | canvas | inspector, with a Code
// tab showing the generated JavaScript. Every edit goes through the store (undoable, autosaved).

import {
  Background,
  BackgroundVariant,
  Controls,
  ReactFlow,
  ReactFlowProvider,
  applyNodeChanges,
  useReactFlow,
  type Connection,
  type Edge,
  type FinalConnectionState,
  type NodeChange,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import "./flow/flow.css";
import fuzzysort from "fuzzysort";
import { Code, Copy, LayoutPanelLeft, Search, Trash2, Wand2, Workflow } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getBackend } from "@/lib/backend";
import type { GraphNode, KeyId } from "@/lib/types";
import { useHotkey } from "@/lib/hotkeys";
import { useDaemonEvent } from "@/state/daemon";
import { useBinding } from "@/state/selectors";
import { editor } from "@/state/store";
import { Button, confirm, EmptyState, IconButton, Input, RefIcon, ScrollArea, Tabs, TabsList, TabsTrigger, toast } from "@/ui";
import { ActionPicker } from "./ActionPicker";
import { ACTION_DEFS, CATEGORIES, categoryColor, nodeDef, validateNode, type NodeContext } from "./catalog";
import { CodeEditor } from "./CodeEditor";
import { useNodeContext } from "./data";
import { FlowNode, type FlowNodeType } from "./flow/FlowNode";
import { tidy } from "./flow/layout";
import { ParamForm } from "./forms/ParamForm";
import { emptyGraph, isTrigger, newEdge, newNode, type Graph } from "./graph";
import { TestRow } from "./MacroSection";

export interface FlowEditorViewProps {
  profileId: string;
  keyId: KeyId;
}

const DRAG_MIME = "application/x-streampad-node";
const nodeTypes = { step: FlowNode };

export function FlowEditorView(props: FlowEditorViewProps) {
  return (
    <ReactFlowProvider>
      <FlowEditor {...props} />
    </ReactFlowProvider>
  );
}

function reachable(g: Graph): Set<string> {
  const seen = new Set<string>();
  const stack = g.nodes.filter((n) => isTrigger(n.kind)).map((n) => n.id);
  while (stack.length) {
    const id = stack.pop()!;
    if (seen.has(id)) continue;
    seen.add(id);
    for (const e of g.edges) if (e.source === id) stack.push(e.target);
  }
  return seen;
}

function FlowEditor({ profileId, keyId }: FlowEditorViewProps) {
  const binding = useBinding(profileId, keyId);
  const ctx = useNodeContext();
  const rf = useReactFlow();
  const macro = binding?.macro;
  const graph: Graph | null = macro?.type === "graph" ? macro : null;
  const [tab, setTab] = useState<"flow" | "code">("flow");
  const [selected, setSelected] = useState<string | null>(null);
  const [compileError, setCompileError] = useState<{ message: string; nodeId?: string } | null>(null);
  const [code, setCode] = useState("");
  const [traced, setTraced] = useState<Record<string, number>>({});
  const [quickAdd, setQuickAdd] = useState<null | { from?: { node: string; port: string }; at: { x: number; y: number } }>(null);
  const [rfNodes, setRfNodes] = useState<FlowNodeType[]>([]);
  const clipboard = useRef<GraphNode[] | null>(null);

  // A key without a graph macro gets an empty one (scripts are edited in the script view).
  useEffect(() => {
    if (!macro) editor.updateBinding(profileId, keyId, (b) => void (b.macro = emptyGraph()), { coalesce: false });
  }, [macro, profileId, keyId]);

  const commit = useCallback(
    (next: Graph, coalesce: string | false = false) =>
      editor.updateBinding(profileId, keyId, (b) => void (b.macro = next), { coalesce }),
    [profileId, keyId],
  );

  // Live validation + generated code (debounced).
  useEffect(() => {
    if (!graph) return;
    const t = setTimeout(async () => {
      const res = await getBackend().compileMacro(graph);
      if (res.ok) {
        setCompileError(null);
        setCode(res.code);
      } else {
        setCompileError({ message: res.error, nodeId: res.nodeId });
      }
    }, 250);
    return () => clearTimeout(t);
  }, [graph]);

  useDaemonEvent("trace", ({ nodeId }) => {
    setTraced((t) => ({ ...t, [nodeId]: Date.now() }));
    setTimeout(() => setTraced((t) => (Date.now() - (t[nodeId] ?? 0) >= 650 ? (({ [nodeId]: _, ...rest }) => rest)(t) : t)), 700);
  });

  const live = useMemo(() => (graph ? reachable(graph) : new Set<string>()), [graph]);

  // Rebuild React Flow nodes whenever the graph (or derived data) changes; keep local drag positions.
  useEffect(() => {
    if (!graph) return;
    setRfNodes((prev) => {
      const prevById = new Map(prev.map((n) => [n.id, n]));
      return graph.nodes.map((n) => {
        const p = prevById.get(n.id);
        const err = compileError?.nodeId === n.id ? compileError.message : live.has(n.id) ? validateNode(n.kind, n.params) : null;
        return {
          id: n.id,
          type: "step" as const,
          position: p?.dragging ? p.position : n.position,
          dragging: p?.dragging,
          selected: n.id === selected,
          deletable: n.kind !== "trigger.press",
          data: { node: n, ctx, error: isTrigger(n.kind) ? null : err, traced: !!traced[n.id], unreachable: !live.has(n.id) },
        };
      });
    });
  }, [graph, ctx, compileError, traced, live, selected]);

  const edges: Edge[] = useMemo(
    () =>
      (graph?.edges ?? []).map((e) => ({
        id: e.id,
        source: e.source,
        sourceHandle: e.sourcePort,
        target: e.target,
        targetHandle: "in",
        animated: !!traced[e.target],
        className: "sp-edge",
      })),
    [graph, traced],
  );

  const onNodesChange = (changes: NodeChange<FlowNodeType>[]) => {
    setRfNodes((ns) => applyNodeChanges(changes, ns));
    for (const c of changes) if (c.type === "select" && c.selected) setSelected(c.id);
    const moved = changes.filter((c) => c.type === "position" && c.dragging === false && c.position);
    if (moved.length && graph) {
      const pos = new Map(moved.map((c) => [(c as { id: string }).id, (c as { position: { x: number; y: number } }).position]));
      commit({ ...graph, nodes: graph.nodes.map((n) => (pos.has(n.id) ? { ...n, position: pos.get(n.id)! } : n)) }, "flow:move");
    }
  };

  const connect = (g: Graph, source: string, port: string, target: string): Graph => {
    if (source === target) return g;
    const edges = g.edges.filter((e) => !(e.source === source && e.sourcePort === port));
    return { ...g, edges: [...edges, newEdge(source, port, target)] };
  };

  const onConnect = (c: Connection) => {
    if (!graph || !c.source || !c.target) return;
    commit(connect(graph, c.source, c.sourceHandle ?? "out", c.target));
  };

  const onConnectEnd = (event: MouseEvent | TouchEvent, state: FinalConnectionState) => {
    if (state.isValid || !state.fromNode || state.fromHandle?.type !== "source") return;
    const pt = "changedTouches" in event ? event.changedTouches[0]! : event;
    setQuickAdd({ from: { node: state.fromNode.id, port: state.fromHandle.id ?? "out" }, at: rf.screenToFlowPosition({ x: pt.clientX, y: pt.clientY }) });
  };

  const addNode = (kind: string, at: { x: number; y: number }, from?: { node: string; port: string }) => {
    if (!graph) return;
    const node = newNode(kind, { x: Math.round(at.x), y: Math.round(at.y - 24) });
    let next: Graph = { ...graph, nodes: [...graph.nodes, node] };
    if (from) next = connect(next, from.node, from.port, node.id);
    commit(next);
    setSelected(node.id);
  };

  /** Click in the palette: place after the selected node (wired to its first free output) or at the centre. */
  const addFromPalette = (kind: string) => {
    if (!graph) return;
    const sel = graph.nodes.find((n) => n.id === selected);
    if (sel) {
      const free = nodeDef(sel.kind).outputs.find((o) => !graph.edges.some((e) => e.source === sel.id && e.sourcePort === o.port));
      addNode(kind, { x: sel.position.x + 280, y: sel.position.y + 24 }, free ? { node: sel.id, port: free.port } : undefined);
    } else {
      const box = document.querySelector(".react-flow")?.getBoundingClientRect();
      addNode(kind, rf.screenToFlowPosition({ x: (box?.left ?? 0) + (box?.width ?? 800) / 2, y: (box?.top ?? 0) + (box?.height ?? 600) / 2 }));
    }
  };

  const removeNodes = (ids: string[]) => {
    if (!graph) return;
    const drop = new Set(ids.filter((id) => graph.nodes.find((n) => n.id === id)?.kind !== "trigger.press"));
    if (!drop.size) return;
    commit({ type: "graph", nodes: graph.nodes.filter((n) => !drop.has(n.id)), edges: graph.edges.filter((e) => !drop.has(e.source) && !drop.has(e.target)) });
    if (selected && drop.has(selected)) setSelected(null);
  };

  const duplicate = (nodes: GraphNode[]) => {
    if (!graph || !nodes.length) return;
    const copies = nodes.filter((n) => n.kind !== "trigger.press").map((n) => newNode(n.kind, { x: n.position.x + 40, y: n.position.y + 40 }, structuredClone(n.params)));
    commit({ ...graph, nodes: [...graph.nodes, ...copies] });
    setSelected(copies[0]?.id ?? null);
  };

  const selectedNodes = () => (graph ? graph.nodes.filter((n) => rfNodes.find((r) => r.id === n.id)?.selected) : []);

  useHotkey("mod+d", () => duplicate(selectedNodes()), { description: "Duplicate node" });
  useHotkey("mod+c", () => void (clipboard.current = selectedNodes()), { description: "Copy nodes" });
  useHotkey("mod+v", () => duplicate(clipboard.current ?? []), { description: "Paste nodes" });
  useHotkey("f", () => void rf.fitView({ duration: 250, padding: 0.2 }), { description: "Fit view" });

  const runTidy = async () => {
    if (!graph) return;
    commit(await tidy(graph));
    setTimeout(() => void rf.fitView({ duration: 300, padding: 0.2 }), 50);
  };

  const toScript = async () => {
    if (!graph) return;
    const res = await getBackend().compileMacro(graph);
    if (!res.ok) return toast.error("Fix the flow first", { description: res.error });
    const ok = await confirm({ title: "Convert to a script?", description: "The flow becomes JavaScript you can edit freely. It can't be turned back into a flow.", confirmLabel: "Convert" });
    if (!ok) return;
    editor.updateBinding(profileId, keyId, (b) => void (b.macro = { type: "script", source: res.code }), { coalesce: false });
    editor.openScript(profileId, keyId);
  };

  if (macro?.type === "script") {
    return (
      <div className="flex flex-1 items-center justify-center bg-bg">
        <EmptyState icon={Code} title="This key runs a script" description="Scripts can't be shown as a flow." action={<Button onClick={() => editor.openScript(profileId, keyId)}>Open the script</Button>} />
      </div>
    );
  }
  if (!graph) return null;

  const sel = graph.nodes.find((n) => n.id === selected) ?? null;
  const issues = graph.nodes
    .filter((n) => live.has(n.id) && !isTrigger(n.kind))
    .map((n) => ({ n, msg: compileError?.nodeId === n.id ? compileError.message : validateNode(n.kind, n.params) }))
    .filter((x) => x.msg);
  const globalError = compileError && !compileError.nodeId ? compileError.message : null;

  return (
    <div className="flex min-h-0 flex-1">
      <Palette onAdd={addFromPalette} />
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex h-11 shrink-0 items-center gap-2 border-b border-line px-3">
          <Tabs value={tab} onValueChange={(v) => setTab(v as "flow" | "code")}>
            <TabsList variant="pill">
              <TabsTrigger value="flow" icon={Workflow}>
                Flow
              </TabsTrigger>
              <TabsTrigger value="code" icon={Code}>
                Code
              </TabsTrigger>
            </TabsList>
          </Tabs>
          <div className="flex-1" />
          {tab === "flow" && (
            <Button size="sm" variant="ghost" icon={LayoutPanelLeft} onClick={() => void runTidy()}>
              Tidy up
            </Button>
          )}
          <Button size="sm" variant="ghost" icon={Wand2} onClick={() => void toScript()}>
            Convert to script
          </Button>
          <div className="w-44">
            <TestRow macro={graph} keyId={keyId} compact />
          </div>
          <Button size="sm" variant="primary" onClick={editor.showMain}>
            Done
          </Button>
        </div>
        {tab === "flow" ? (
          <div
            className="sp-flow relative min-h-0 flex-1"
            onDragOver={(e) => {
              if (e.dataTransfer.types.includes(DRAG_MIME)) {
                e.preventDefault();
                e.dataTransfer.dropEffect = "copy";
              }
            }}
            onDrop={(e) => {
              const kind = e.dataTransfer.getData(DRAG_MIME);
              if (!kind) return;
              e.preventDefault();
              addNode(kind, rf.screenToFlowPosition({ x: e.clientX, y: e.clientY }));
            }}
          >
            <ReactFlow<FlowNodeType>
              nodes={rfNodes}
              edges={edges}
              nodeTypes={nodeTypes}
              onNodesChange={onNodesChange}
              onEdgesDelete={(del) => commit({ ...graph, edges: graph.edges.filter((e) => !del.some((d) => d.id === e.id)) })}
              onNodesDelete={(del) => removeNodes(del.map((d) => d.id))}
              onConnect={onConnect}
              onConnectEnd={onConnectEnd}
              onPaneClick={() => setSelected(null)}
              isValidConnection={(c) => c.source !== c.target && c.targetHandle === "in"}
              deleteKeyCode={["Delete", "Backspace"]}
              snapToGrid
              snapGrid={[8, 8]}
              fitView
              fitViewOptions={{ padding: 0.3, maxZoom: 1.1 }}
              minZoom={0.3}
              maxZoom={1.75}
              proOptions={{ hideAttribution: true }}
              defaultEdgeOptions={{ type: "default" }}
              colorMode="system"
            >
              <Background variant={BackgroundVariant.Dots} gap={20} size={1.2} color="var(--sp-line-strong)" />
              <Controls showInteractive={false} position="bottom-left" />
            </ReactFlow>
            {graph.nodes.length <= 1 && (
              <div className="pointer-events-none absolute inset-x-0 bottom-10 flex justify-center">
                <p className="rounded-full border border-line bg-panel px-4 py-2 text-sm text-muted shadow-popover">
                  Drag from <span className="text-fg">When pressed</span>'s handle, or pick a step on the left.
                </p>
              </div>
            )}
          </div>
        ) : (
          <div className="min-h-0 flex-1 overflow-auto bg-panel">
            {compileError ? (
              <p className="m-4 rounded-md bg-danger-soft px-3 py-2 text-sm text-danger">Can't generate code yet: {compileError.message}</p>
            ) : (
              <>
                <div className="flex items-center gap-2 border-b border-line-subtle px-4 py-2 text-sm text-muted">
                  This is the JavaScript the flow runs as. Read-only — convert to a script to edit it.
                  <div className="flex-1" />
                  <IconButton icon={Copy} label="Copy code" size="sm" onClick={() => void navigator.clipboard.writeText(code).then(() => toast("Copied"))} />
                </div>
                <CodeEditor value={code} readOnly lineNumbers api={false} />
              </>
            )}
          </div>
        )}
      </div>
      <aside className="flex w-(--sp-inspector-w) shrink-0 flex-col border-l border-line bg-panel">
        {sel ? (
          <NodeInspector node={sel} ctx={ctx} onChange={(p) => commit({ ...graph, nodes: graph.nodes.map((n) => (n.id === sel.id ? { ...n, params: { ...n.params, ...p } } : n)) }, `flow:${sel.id}:${Object.keys(p).join(",")}`)} onDelete={() => removeNodes([sel.id])} error={compileError?.nodeId === sel.id ? compileError.message : validateNode(sel.kind, sel.params)} />
        ) : (
          <ScrollArea className="min-h-0 flex-1">
            <div className="flex flex-col gap-4 p-4">
              <h2 className="text-md font-semibold">Flow</h2>
              {globalError && <p className="rounded-md bg-danger-soft px-3 py-2 text-sm text-danger">{globalError}</p>}
              {issues.length > 0 ? (
                <div className="flex flex-col gap-1">
                  <span className="text-sm font-medium text-muted">Needs attention</span>
                  {issues.map(({ n, msg }) => (
                    <button key={n.id} type="button" onClick={() => { setSelected(n.id); void rf.fitView({ nodes: [{ id: n.id }], duration: 250, maxZoom: 1.2 }); }} className="flex items-center gap-2 rounded-sm px-2 py-1.5 text-left text-sm hover:bg-control">
                      <RefIcon iconRef={`lucide:${nodeDef(n.kind).icon}`} size={14} className="text-warning" />
                      <span className="font-medium">{nodeDef(n.kind).title}</span>
                      <span className="truncate text-muted">{msg}</span>
                    </button>
                  ))}
                </div>
              ) : (
                !globalError && <p className="text-sm text-success">Ready to run.</p>
              )}
              <div className="flex flex-col gap-2 text-sm text-muted">
                <p>Select a step to edit it. Drag from an output into empty space to add a connected step.</p>
                <p>
                  <span className="text-fg">If / else</span> branches on the focused app, a variable, the time… <span className="text-fg">Repeat</span> loops. Unconnected steps are dimmed and never run.
                </p>
              </div>
            </div>
          </ScrollArea>
        )}
      </aside>
      <ActionPicker
        open={quickAdd !== null}
        onOpenChange={(o) => !o && setQuickAdd(null)}
        title="Add a connected step"
        onPick={(kind) => quickAdd && addNode(kind, quickAdd.at, quickAdd.from)}
      />
    </div>
  );
}

function NodeInspector({ node, ctx, onChange, onDelete, error }: { node: GraphNode; ctx: NodeContext; onChange(p: Record<string, unknown>): void; onDelete(): void; error: string | null }) {
  const def = nodeDef(node.kind);
  const color = categoryColor(def.category);
  return (
    <>
      <div className="flex items-center gap-2.5 border-b border-line px-4 py-3">
        <span className="flex size-8 items-center justify-center rounded-md" style={{ background: `color-mix(in srgb, ${color} 16%, transparent)`, color }}>
          <RefIcon iconRef={`lucide:${def.icon}`} size={16} />
        </span>
        <div className="min-w-0 flex-1">
          <h2 className="truncate text-md font-semibold">{def.title}</h2>
          <p className="truncate text-xs text-muted">{def.summary(node.params, ctx)}</p>
        </div>
        {node.kind !== "trigger.press" && <IconButton icon={Trash2} label="Delete step" shortcut="delete" onClick={onDelete} />}
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <div className="flex flex-col gap-4 p-4">
          <p className="text-sm text-muted">{def.description}</p>
          {error && !isTrigger(node.kind) && <p className="rounded-md bg-warning-soft px-3 py-2 text-sm text-warning">{error}</p>}
          {!isTrigger(node.kind) && <ParamForm key={node.id} kind={node.kind} params={node.params} onChange={onChange} />}
        </div>
      </ScrollArea>
    </>
  );
}

function Palette({ onAdd }: { onAdd(kind: string): void }) {
  const [q, setQ] = useState("");
  const list = useMemo(() => (q.trim() ? fuzzysort.go(q, ACTION_DEFS, { keys: ["title", "description", "category", "keywords"], threshold: 0.2 }).map((r) => r.obj) : ACTION_DEFS), [q]);
  return (
    <aside className="flex w-60 shrink-0 flex-col border-r border-line bg-panel">
      <div className="p-2">
        <Input icon={Search} placeholder="Find a step" value={q} onChange={(e) => setQ(e.currentTarget.value)} />
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <div className="flex flex-col px-2 pb-3">
          {CATEGORIES.map((c) => {
            const items = list.filter((d) => d.category === c.id);
            if (!items.length) return null;
            return (
              <div key={c.id} className="mb-2">
                <div className="flex items-center gap-1.5 px-2 pb-1 pt-2 text-xs font-medium text-faint">
                  <span className="size-1.5 rounded-full" style={{ background: c.color }} />
                  {c.id}
                </div>
                {items.map((d) => (
                  <button
                    key={d.kind}
                    type="button"
                    draggable
                    onDragStart={(e) => {
                      e.dataTransfer.setData(DRAG_MIME, d.kind);
                      e.dataTransfer.effectAllowed = "copy";
                    }}
                    onClick={() => onAdd(d.kind)}
                    title={d.description}
                    className="flex w-full cursor-grab items-center gap-2 rounded-sm px-2 py-1.5 text-left text-base text-muted transition-quick hover:bg-control hover:text-fg active:cursor-grabbing"
                  >
                    <span style={{ color: c.color }} className="flex"><RefIcon iconRef={`lucide:${d.icon}`} size={14} /></span>
                    <span className="truncate">{d.title}</span>
                  </button>
                ))}
              </div>
            );
          })}
        </div>
      </ScrollArea>
    </aside>
  );
}
