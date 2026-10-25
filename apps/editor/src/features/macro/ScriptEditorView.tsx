// Full-window JavaScript editor for one key's macro: code | API reference + console.

import type { EditorView } from "@codemirror/view";
import fuzzysort from "fuzzysort";
import { BookOpen, Code, Search, SquareTerminal, Trash } from "lucide-react";
import { useMemo, useRef, useState } from "react";
import { cn } from "@/lib/cn";
import type { KeyId } from "@/lib/types";
import { clearLogs, useDaemon } from "@/state/daemon";
import { useBinding } from "@/state/selectors";
import { editor } from "@/state/store";
import { Button, EmptyState, Input, ScrollArea, Tabs, TabsContent, TabsList, TabsTrigger } from "@/ui";
import { API, SNIPPETS } from "./api";
import { CodeEditor, syntaxError } from "./CodeEditor";
import { TestRow } from "./MacroSection";

export interface ScriptEditorViewProps {
  profileId: string;
  keyId: KeyId;
}

const STARTER = `// Runs when the key is pressed. The whole StreamPad API is available — see the reference on the right.
// Example: different shortcut per focused app.
if (window.isActive("firefox, chromium")) {
  press("ctrl+t");
} else {
  launch("firefox");
}
`;

export function ScriptEditorView({ profileId, keyId }: ScriptEditorViewProps) {
  const binding = useBinding(profileId, keyId);
  const macro = binding?.macro;
  const view = useRef<EditorView | null>(null);

  if (macro && macro.type !== "script") {
    return (
      <div className="flex flex-1 items-center justify-center bg-bg">
        <EmptyState
          icon={Code}
          title="This key uses steps"
          description="Convert it to a script from the inspector's ⋯ menu or the flow editor."
          action={<Button onClick={() => editor.openFlow(profileId, keyId)}>Open the flow editor</Button>}
        />
      </div>
    );
  }

  const source = macro?.source ?? STARTER;
  const setSource = (s: string) => editor.updateBinding(profileId, keyId, (b) => void (b.macro = { type: "script", source: s }), { coalesce: `script:${profileId}:${keyId}` });

  return (
    <div className="flex min-h-0 flex-1">
      <ScriptWorkbench source={source} onChange={setSource} viewRef={view} keyId={keyId} />
    </div>
  );
}

/** Code editor + reference/console side panel. Reused by Settings › Scripting for the shared library. */
export function ScriptWorkbench({
  source,
  onChange,
  viewRef,
  keyId,
  showTest = true,
  doneLabel = "Done",
}: {
  source: string;
  onChange(s: string): void;
  viewRef?: React.RefObject<EditorView | null>;
  keyId: KeyId | null;
  showTest?: boolean;
  doneLabel?: string | null;
}) {
  const local = useRef<EditorView | null>(null);
  const ref = viewRef ?? local;
  const err = useMemo(() => syntaxError(source), [source]);
  const insert = (text: string) => {
    const v = ref.current;
    if (!v) return;
    const { from, to } = v.state.selection.main;
    v.dispatch({ changes: { from, to, insert: text }, selection: { anchor: from + text.length } });
    v.focus();
  };

  return (
    <>
      <div className="flex min-w-0 flex-1 flex-col">
        <div className="flex h-11 shrink-0 items-center gap-2 border-b border-line px-3">
          <Code size={15} className="text-muted" />
          <span className="text-sm text-muted">JavaScript</span>
          {err ? (
            <span className="truncate text-sm text-danger">
              Line {err.line}:{err.column} — {err.message}
            </span>
          ) : (
            <span className="text-sm text-faint">No syntax errors</span>
          )}
          <div className="flex-1" />
          {showTest && (
            <div className="w-48">
              <TestRow macro={{ type: "script", source }} keyId={keyId} compact />
            </div>
          )}
          {doneLabel && (
            <Button size="sm" variant="primary" onClick={editor.showMain}>
              {doneLabel}
            </Button>
          )}
        </div>
        <div className="min-h-0 flex-1 bg-panel">
          <CodeEditor value={source} onChange={onChange} lineNumbers className="h-full" onView={(v) => (ref.current = v)} />
        </div>
      </div>
      <aside className="flex w-(--sp-inspector-w) shrink-0 flex-col border-l border-line bg-panel">
        <Tabs defaultValue="reference" className="flex min-h-0 flex-1 flex-col">
          <div className="border-b border-line px-3 py-2">
            <TabsList variant="pill">
              <TabsTrigger value="reference" icon={BookOpen}>
                Reference
              </TabsTrigger>
              <TabsTrigger value="console" icon={SquareTerminal}>
                Console
              </TabsTrigger>
            </TabsList>
          </div>
          <TabsContent value="reference" className="flex min-h-0 flex-1 flex-col">
            <Reference onInsert={insert} />
          </TabsContent>
          <TabsContent value="console" className="flex min-h-0 flex-1 flex-col">
            <Console />
          </TabsContent>
        </Tabs>
      </aside>
    </>
  );
}

function Reference({ onInsert }: { onInsert(text: string): void }) {
  const [q, setQ] = useState("");
  const items = useMemo(() => (q.trim() ? fuzzysort.go(q, API, { keys: ["name", "doc", "group"], threshold: 0.2 }).map((r) => r.obj) : API), [q]);
  const groups = [...new Set(items.map((a) => a.group))];
  return (
    <>
      <div className="p-2">
        <Input icon={Search} placeholder="Search the API" value={q} onChange={(e) => setQ(e.currentTarget.value)} />
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <div className="flex flex-col gap-3 px-2 pb-4">
          {!q && (
            <div>
              <div className="px-2 pb-1 text-xs font-medium text-faint">Snippets</div>
              {SNIPPETS.map((s) => (
                <button key={s.label} type="button" onClick={() => onInsert(`${s.code}\n`)} className="w-full rounded-sm px-2 py-1.5 text-left hover:bg-control">
                  <div className="text-base text-fg">{s.label}</div>
                  <div className="text-xs text-muted">{s.detail}</div>
                </button>
              ))}
            </div>
          )}
          {groups.map((g) => (
            <div key={g}>
              <div className="px-2 pb-1 text-xs font-medium text-faint">{g}</div>
              {items
                .filter((a) => a.group === g)
                .map((a) => (
                  <button key={a.name} type="button" onClick={() => onInsert(a.insert)} title="Insert at cursor" className="w-full rounded-sm px-2 py-1.5 text-left hover:bg-control">
                    <div className="truncate font-code text-xs text-accent-text">{a.signature}</div>
                    <div className="text-xs text-muted">{a.doc}</div>
                  </button>
                ))}
            </div>
          ))}
        </div>
      </ScrollArea>
    </>
  );
}

function Console() {
  const logs = useDaemon((s) => s.logs);
  return (
    <>
      <div className="flex items-center justify-between px-3 py-2 text-xs text-faint">
        <span>log() output and errors from every macro</span>
        <Button size="sm" variant="ghost" icon={Trash} onClick={clearLogs}>
          Clear
        </Button>
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <div className="flex flex-col px-3 pb-3 font-code text-xs">
          {logs.length === 0 && <p className="py-6 text-center font-sans text-sm text-faint">Nothing yet. Press Test or a key.</p>}
          {logs.map((l) => (
            <div key={l.id} className={cn("whitespace-pre-wrap border-b border-line-subtle py-1", l.level === "error" ? "text-danger" : l.level === "warn" ? "text-warning" : "text-fg")}>
              <span className="mr-2 text-faint">{new Date(l.at).toLocaleTimeString([], { hour12: false })}</span>
              {l.message}
            </div>
          ))}
          <div key={logs.length} ref={(el) => void el?.scrollIntoView({ block: "nearest" })} />
        </div>
      </ScrollArea>
    </>
  );
}
