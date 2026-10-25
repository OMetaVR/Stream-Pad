// The inspector's macro block: choose an action, edit simple macros as a step list, jump to the flow or
// script editor for everything else, test it, and set how the key behaves when held / pressed again.

import { DndContext, PointerSensor, closestCenter, useSensor, useSensors, type DragEndEvent } from "@dnd-kit/core";
import { SortableContext, arrayMove, useSortable, verticalListSortingStrategy } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { AlertCircle, ChevronDown, Code, GripVertical, MoreHorizontal, Play, Plus, Square, Trash2, Workflow, Zap } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { getBackend } from "@/lib/backend";
import { cn } from "@/lib/cn";
import type { Binding, GraphNode, KeyId, Macro } from "@/lib/types";
import { onDaemonEvent, useDaemon, useDaemonEvent, useRunLog, type DaemonEventData } from "@/state/daemon";
import { editor, useRequest } from "@/state/store";
import {
  Button,
  Checkbox,
  confirm,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  Field,
  IconButton,
  RefIcon,
  Select,
  toast,
} from "@/ui";
import { ActionPicker } from "./ActionPicker";
import { categoryColor, nodeDef, validateNode, type NodeContext, type Params } from "./catalog";
import { useNodeContext } from "./data";
import { ParamForm } from "./forms/ParamForm";
import { chainFrom, emptyGraph, firstAction, isLinear, newNode, setChain, type Graph } from "./graph";

export interface MacroSectionProps {
  profileId: string;
  keyId: KeyId;
  binding: Binding;
  onChange: (next: Binding) => void;
}

type Chain = "trigger.press" | "trigger.release";

export function MacroSection({ profileId, keyId, binding, onChange }: MacroSectionProps) {
  const ctx = useNodeContext();
  const [picker, setPicker] = useState<null | { chain: Chain; mode: "first" | "append" }>(null);
  const [openStep, setOpenStep] = useState<string | null>(null);
  const macro = binding.macro;

  useRequest("actionPicker", (payload) => {
    const p = payload as { profileId?: string; keyId?: KeyId } | undefined;
    if (!p || (p.profileId === profileId && p.keyId === keyId)) setPicker({ chain: "trigger.press", mode: macro ? "append" : "first" });
  });

  const setMacro = (next: Macro | null, coalesce?: string) =>
    editor.updateBinding(profileId, keyId, (b) => {
      // Labels/icons that came from the first action keep following it until the user changes them.
      const before = b.macro?.type === "graph" ? firstAction(b.macro) : undefined;
      const after = next?.type === "graph" ? firstAction(next) : undefined;
      if (before && after && before.id === after.id) {
        const def = nodeDef(after.kind);
        const oldLabel = def.defaultLabel?.(before.params, ctx) ?? "";
        const oldIcon = def.defaultIcon?.(before.params, ctx) ?? null;
        if (b.label === "" || b.label === oldLabel) b.label = def.defaultLabel?.(after.params, ctx) ?? b.label;
        if (b.icon === null || b.icon === oldIcon) b.icon = def.defaultIcon?.(after.params, ctx) ?? b.icon;
      }
      b.macro = next;
    }, { coalesce: coalesce ?? false });

  const addAction = (kind: string, chain: Chain, mode: "first" | "append") => {
    const node = newNode(kind);
    const graph: Graph = macro?.type === "graph" ? macro : emptyGraph();
    const current = chainFrom(graph, chain) ?? [];
    const next = setChain(graph, chain, [...current, node]);
    if (mode === "first" && chain === "trigger.press") {
      const label = binding.label || nodeDef(kind).defaultLabel?.(node.params, ctx) || "";
      const icon = binding.icon ?? nodeDef(kind).defaultIcon?.(node.params, ctx) ?? null;
      onChange({ ...binding, macro: next, label, icon });
    } else {
      setMacro(next);
    }
    setOpenStep(node.id);
  };

  return (
    <section className="flex flex-col gap-3" aria-label="Action">
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-medium text-muted">Action</h3>
        {macro && <MacroMenu profileId={profileId} keyId={keyId} macro={macro} onClear={() => onChange({ ...binding, macro: null })} />}
      </div>

      {!macro && (
        <button
          type="button"
          onClick={() => setPicker({ chain: "trigger.press", mode: "first" })}
          className="group flex flex-col items-center gap-2 rounded-lg border border-dashed border-line-strong px-4 py-6 text-center transition-quick hover:border-accent-line hover:bg-accent-softer"
        >
          <span className="flex size-10 items-center justify-center rounded-full bg-accent-soft text-accent-text transition-quick group-hover:scale-105">
            <Zap size={18} />
          </span>
          <span className="text-base font-medium text-fg">Choose an action</span>
          <span className="text-sm text-muted">Media, apps, shortcuts, websites, windows…</span>
        </button>
      )}

      {macro?.type === "graph" && isLinear(macro) && (
        <>
          <StepList
            graph={macro}
            chain="trigger.press"
            ctx={ctx}
            openStep={openStep}
            setOpenStep={setOpenStep}
            onGraph={setMacro}
            onAdd={() => setPicker({ chain: "trigger.press", mode: "append" })}
          />
          <ReleaseSection graph={macro} ctx={ctx} openStep={openStep} setOpenStep={setOpenStep} onGraph={setMacro} onAdd={() => setPicker({ chain: "trigger.release", mode: "append" })} />
        </>
      )}

      {macro?.type === "graph" && !isLinear(macro) && <FlowSummary graph={macro} ctx={ctx} onOpen={() => editor.openFlow(profileId, keyId)} />}

      {macro?.type === "script" && (
        <button
          type="button"
          onClick={() => editor.openScript(profileId, keyId)}
          className="group overflow-hidden rounded-lg border border-line bg-raised text-left transition-quick hover:border-line-strong"
        >
          <pre className="max-h-32 overflow-hidden px-3 py-2.5 font-code text-xs leading-relaxed text-muted">{macro.source.trim() || "// empty script"}</pre>
          <div className="flex items-center gap-2 border-t border-line px-3 py-2 text-sm text-fg">
            <Code size={14} className="text-accent-text" /> Edit script
          </div>
        </button>
      )}

      {macro && <TestRow macro={macro} keyId={keyId} />}

      {macro && (
        <div className="flex flex-col gap-3 border-t border-line-subtle pt-4">
          <Checkbox
            checked={binding.repeat}
            onCheckedChange={(v) => editor.updateBinding(profileId, keyId, (b) => void (b.repeat = v === true), { coalesce: false })}
            label="Repeat while held"
          />
          <Field label="If pressed while still running">
            <Select
              value={binding.concurrency}
              onValueChange={(v) => editor.updateBinding(profileId, keyId, (b) => void (b.concurrency = v as Binding["concurrency"]), { coalesce: false })}
              options={[
                { value: "ignore", label: "Ignore the press" },
                { value: "restart", label: "Restart it" },
                { value: "parallel", label: "Run another copy" },
              ]}
            />
          </Field>
        </div>
      )}

      <ActionPicker
        open={picker !== null}
        onOpenChange={(o) => !o && setPicker(null)}
        linearOnly={macro?.type === "graph" ? isLinear(macro) : true}
        title={picker?.mode === "append" ? "Add a step" : "Choose an action"}
        onPick={(kind) => {
          if (!picker) return;
          if (["flow.if", "flow.repeat", "flow.stop"].includes(kind)) {
            addAction(kind, picker.chain, picker.mode);
            editor.openFlow(profileId, keyId);
            return;
          }
          addAction(kind, picker.chain, picker.mode);
        }}
      />
    </section>
  );
}

function MacroMenu({ profileId, keyId, macro, onClear }: { profileId: string; keyId: KeyId; macro: Macro; onClear(): void }) {
  const toScript = async () => {
    if (macro.type !== "graph") return;
    const res = await getBackend().compileMacro(macro);
    if (!res.ok) {
      toast.error("Fix the action first", { description: res.error });
      return;
    }
    const ok = await confirm({
      title: "Convert to a script?",
      description: "You'll get the same macro as JavaScript you can edit freely. It can't be turned back into steps.",
      confirmLabel: "Convert",
    });
    if (!ok) return;
    editor.updateBinding(profileId, keyId, (b) => void (b.macro = { type: "script", source: res.code }), { coalesce: false });
    editor.openScript(profileId, keyId);
  };
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <IconButton icon={MoreHorizontal} label="More" size="sm" />
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        {macro.type === "graph" && (
          <DropdownMenuItem icon={Workflow} onSelect={() => editor.openFlow(profileId, keyId)}>
            Open flow editor
          </DropdownMenuItem>
        )}
        {macro.type === "graph" ? (
          <DropdownMenuItem icon={Code} onSelect={() => void toScript()}>
            Convert to script…
          </DropdownMenuItem>
        ) : (
          <DropdownMenuItem icon={Code} onSelect={() => editor.openScript(profileId, keyId)}>
            Edit script
          </DropdownMenuItem>
        )}
        <DropdownMenuSeparator />
        <DropdownMenuItem icon={Trash2} destructive onSelect={onClear}>
          Remove action
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

interface StepListProps {
  graph: Graph;
  chain: Chain;
  ctx: NodeContext;
  openStep: string | null;
  setOpenStep(id: string | null): void;
  onGraph(g: Graph, coalesce?: string): void;
  onAdd(): void;
}

function StepList({ graph, chain, ctx, openStep, setOpenStep, onGraph, onAdd }: StepListProps) {
  const steps = useMemo(() => chainFrom(graph, chain) ?? [], [graph, chain]);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));

  const onDragEnd = (e: DragEndEvent) => {
    if (!e.over || e.active.id === e.over.id) return;
    const from = steps.findIndex((s) => s.id === e.active.id);
    const to = steps.findIndex((s) => s.id === e.over!.id);
    onGraph(setChain(graph, chain, arrayMove(steps, from, to)));
  };
  const update = (id: string, patch: Params) =>
    onGraph(
      { ...graph, nodes: graph.nodes.map((n) => (n.id === id ? { ...n, params: { ...n.params, ...patch } } : n)) },
      `step:${id}:${Object.keys(patch).join(",")}`,
    );
  const remove = (id: string) => onGraph(setChain(graph, chain, steps.filter((s) => s.id !== id)));

  return (
    <div className="flex flex-col gap-1.5">
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
        <SortableContext items={steps.map((s) => s.id)} strategy={verticalListSortingStrategy}>
          {steps.map((s, i) => (
            <StepRow key={s.id} node={s} index={i} ctx={ctx} open={openStep === s.id} onToggle={() => setOpenStep(openStep === s.id ? null : s.id)} onChange={(p) => update(s.id, p)} onRemove={() => remove(s.id)} />
          ))}
        </SortableContext>
      </DndContext>
      <Button variant="ghost" size="sm" icon={Plus} className="self-start text-muted" onClick={onAdd}>
        {steps.length ? "Add step" : "Add an action"}
      </Button>
    </div>
  );
}

function StepRow({ node, index, ctx, open, onToggle, onChange, onRemove }: { node: GraphNode; index: number; ctx: NodeContext; open: boolean; onToggle(): void; onChange(p: Params): void; onRemove(): void }) {
  const def = nodeDef(node.kind);
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: node.id });
  const error = validateNode(node.kind, node.params);
  const color = categoryColor(def.category);
  const compact = node.kind === "delay";

  return (
    <div
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      className={cn("rounded-md border bg-raised transition-[border-color,box-shadow]", open ? "border-line-strong" : "border-line", isDragging && "z-10 shadow-popover")}
    >
      <div className={cn("group flex items-center gap-2 pl-1 pr-1.5", compact ? "h-8" : "h-11")}>
        <button type="button" className="flex h-full w-5 cursor-grab items-center justify-center text-faint opacity-0 transition-quick group-hover:opacity-100 active:cursor-grabbing" aria-label={`Reorder step ${index + 1}`} {...attributes} {...listeners}>
          <GripVertical size={14} />
        </button>
        <button type="button" onClick={onToggle} className="flex min-w-0 flex-1 items-center gap-2.5 text-left" aria-expanded={open}>
          <span className={cn("flex shrink-0 items-center justify-center rounded-sm", compact ? "size-5" : "size-7")} style={{ background: `color-mix(in srgb, ${color} 16%, transparent)`, color }}>
            <RefIcon iconRef={`lucide:${def.icon}`} size={compact ? 12 : 15} />
          </span>
          <span className="min-w-0 flex-1">
            {!compact && <span className="block truncate text-base font-medium text-fg">{def.title}</span>}
            <span className={cn("block truncate", compact ? "text-sm text-muted" : "text-xs text-muted")}>{compact ? `Wait ${def.summary(node.params, ctx)}` : def.summary(node.params, ctx)}</span>
          </span>
          {error && <AlertCircle size={14} className="shrink-0 text-warning" aria-label={error} />}
          <ChevronDown size={14} className={cn("shrink-0 text-faint transition-quick", open && "rotate-180")} />
        </button>
        <IconButton icon={Trash2} label="Remove step" size="sm" className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100" onClick={onRemove} />
      </div>
      {open && (
        <div className="border-t border-line-subtle px-3 pb-3 pt-3">
          {error && <p className="mb-3 text-sm text-warning">{error}</p>}
          <ParamForm kind={node.kind} params={node.params} onChange={onChange} autoFocus />
          <p className="mt-3 text-xs text-faint">{def.description}</p>
        </div>
      )}
    </div>
  );
}

function ReleaseSection(props: Omit<StepListProps, "chain">) {
  const steps = chainFrom(props.graph, "trigger.release") ?? [];
  const [expanded, setExpanded] = useState(steps.length > 0);
  useEffect(() => {
    if (steps.length > 0) setExpanded(true);
  }, [steps.length]);
  return (
    <div className="flex flex-col gap-2">
      <button type="button" onClick={() => setExpanded((e) => !e)} className="flex items-center gap-1.5 self-start text-sm text-muted hover:text-fg">
        <ChevronDown size={14} className={cn("transition-quick", !expanded && "-rotate-90")} />
        On release {steps.length > 0 && <span className="text-faint">· {steps.length}</span>}
      </button>
      {expanded && (
        <div className="flex flex-col gap-1.5 pl-1">
          {steps.length === 0 && <p className="text-sm text-faint">Runs when the key comes back up — e.g. release a push-to-talk key.</p>}
          <StepList {...props} chain="trigger.release" />
        </div>
      )}
    </div>
  );
}

function FlowSummary({ graph, ctx, onOpen }: { graph: Graph; ctx: NodeContext; onOpen(): void }) {
  const actions = graph.nodes.filter((n) => !n.kind.startsWith("trigger."));
  const first = firstAction(graph);
  return (
    <button type="button" onClick={onOpen} className="group flex flex-col gap-2 rounded-lg border border-line bg-raised p-3 text-left transition-quick hover:border-line-strong">
      <div className="flex flex-wrap gap-1">
        {actions.slice(0, 8).map((n) => {
          const def = nodeDef(n.kind);
          const color = categoryColor(def.category);
          return (
            <span key={n.id} className="flex size-7 items-center justify-center rounded-sm" title={`${def.title}: ${def.summary(n.params, ctx)}`} style={{ background: `color-mix(in srgb, ${color} 16%, transparent)`, color }}>
              <RefIcon iconRef={`lucide:${def.icon}`} size={14} />
            </span>
          );
        })}
        {actions.length > 8 && <span className="flex h-7 items-center px-1 text-xs text-faint">+{actions.length - 8}</span>}
      </div>
      <div className="text-sm text-muted">
        {actions.length} steps{first ? ` · starts with ${nodeDef(first.kind).summary(first.params, ctx)}` : ""}
      </div>
      <div className="flex items-center gap-2 text-sm text-fg">
        <Workflow size={14} className="text-accent-text" /> Open flow editor
      </div>
    </button>
  );
}

type RunStatus = DaemonEventData<"run">;

// Run events can arrive before testMacro() resolves with the run id, so keep the latest per run.
const lastRunEvent = new Map<number, RunStatus>();
onDaemonEvent("run", (ev) => {
  lastRunEvent.set(ev.runId, ev);
  if (lastRunEvent.size > 50) lastRunEvent.delete(lastRunEvent.keys().next().value!);
});

/** "Test" — runs the (possibly unsaved) macro through the daemon and shows the outcome inline. */
export function TestRow({ macro, keyId, compact }: { macro: Macro; keyId: KeyId | null; compact?: boolean }) {
  const [runId, setRunId] = useState<number | null>(null);
  const [status, setStatus] = useState<RunStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const connected = useDaemon((s) => s.status.kind === "connected");
  const logs = useRunLog(runId);

  useDaemonEvent("run", (ev) => {
    if (ev.runId === runId) setStatus(ev);
  });

  const start = async () => {
    setError(null);
    setStatus(null);
    try {
      const id = await getBackend().testMacro(macro, keyId ?? undefined);
      setRunId(id);
      setStatus(lastRunEvent.get(id) ?? null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const running = runId != null && (!status || status.status === "started");
  const lastLog = logs.at(-1);
  const failure = error ?? (status?.status === "failed" ? status.error ?? "Failed" : null);
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2">
        {running ? (
          <Button size="sm" icon={Square} onClick={() => runId != null && void getBackend().cancelRun(runId)}>
            Stop
          </Button>
        ) : (
          <Button size="sm" icon={Play} disabled={!connected} onClick={() => void start()} title={connected ? "Run it now" : "Start the background service to test"}>
            Test{compact ? "" : " now"}
          </Button>
        )}
        <span className="min-w-0 flex-1 truncate text-sm text-muted">
          {!connected
            ? "Background service not running"
            : running
              ? lastLog?.message ?? "Running…"
              : status?.status === "finished"
                ? <span className="text-success">Done{lastLog ? ` · ${lastLog.message}` : ""}</span>
                : status?.status === "cancelled"
                  ? "Stopped"
                  : status?.status === "failed"
                    ? <span className="text-danger">Failed</span>
                    : null}
        </span>
      </div>
      {failure && <p className="whitespace-pre-wrap rounded-sm bg-danger-soft px-2 py-1.5 font-code text-xs text-danger">{failure}</p>}
    </div>
  );
}
