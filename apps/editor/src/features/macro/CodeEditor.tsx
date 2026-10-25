// CodeMirror 6 editor for macro JavaScript: StreamPad API autocomplete + hover docs, snippets, acorn syntax
// lint, and a theme built from the design tokens. Controlled: `value` in, `onChange` out.

import { autocompletion, closeBrackets, closeBracketsKeymap, completionKeymap, snippetCompletion, type CompletionContext, type CompletionResult } from "@codemirror/autocomplete";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { javascript } from "@codemirror/lang-javascript";
import { bracketMatching, HighlightStyle, indentOnInput, syntaxHighlighting } from "@codemirror/language";
import { linter, lintGutter, type Diagnostic } from "@codemirror/lint";
import { EditorState, type Extension } from "@codemirror/state";
import { EditorView, hoverTooltip, keymap, lineNumbers, placeholder as placeholderExt } from "@codemirror/view";
import { tags as t } from "@lezer/highlight";
import { parse } from "acorn";
import { useEffect, useRef } from "react";
import { API, API_BY_NAME, SNIPPETS } from "./api";

export interface CodeEditorProps {
  value: string;
  onChange?(value: string): void;
  readOnly?: boolean;
  /** StreamPad API completions, hover docs and lint (off for plain JS like the generated-code view). */
  api?: boolean;
  lineNumbers?: boolean;
  minHeight?: number;
  maxHeight?: number;
  placeholder?: string;
  className?: string;
  /** Receives the view (for inserting text from the reference panel). */
  onView?(view: EditorView | null): void;
}

const theme = EditorView.theme({
  "&": { color: "var(--sp-fg)", backgroundColor: "transparent", fontSize: "12.5px", height: "100%" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { fontFamily: "var(--sp-font-mono)", lineHeight: "1.65" },
  ".cm-content": { padding: "10px 0", caretColor: "var(--sp-accent)" },
  ".cm-line": { padding: "0 12px" },
  ".cm-gutters": { backgroundColor: "transparent", color: "var(--sp-faint)", border: "none" },
  ".cm-lineNumbers .cm-gutterElement": { padding: "0 6px 0 12px", minWidth: "32px" },
  ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "color-mix(in srgb, var(--sp-control) 45%, transparent)" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection": { backgroundColor: "var(--sp-accent-soft) !important" },
  ".cm-cursor": { borderLeftColor: "var(--sp-accent)" },
  ".cm-placeholder": { color: "var(--sp-faint)" },
  ".cm-tooltip": {
    backgroundColor: "var(--sp-raised)",
    color: "var(--sp-fg)",
    border: "1px solid var(--sp-line)",
    borderRadius: "8px",
    boxShadow: "var(--sp-shadow-popover)",
    overflow: "hidden",
  },
  ".cm-tooltip-autocomplete > ul": { fontFamily: "var(--sp-font-mono)", fontSize: "12px", maxHeight: "16em" },
  ".cm-tooltip-autocomplete > ul > li": { padding: "3px 10px !important" },
  ".cm-tooltip-autocomplete > ul > li[aria-selected]": { backgroundColor: "var(--sp-control-active)", color: "var(--sp-fg)" },
  ".cm-completionDetail": { color: "var(--sp-muted)", fontStyle: "normal", marginLeft: "10px" },
  ".cm-completionInfo": { padding: "8px 10px", maxWidth: "320px", fontFamily: "var(--sp-font-sans)", fontSize: "12px" },
  ".cm-diagnostic-error": { borderLeft: "3px solid var(--sp-danger)" },
  ".cm-lintRange-error": { backgroundImage: "none", textDecoration: "underline wavy var(--sp-danger)", textUnderlineOffset: "3px" },
  ".cm-sp-doc": { padding: "8px 10px", maxWidth: "360px", fontSize: "12px", lineHeight: "1.5" },
  ".cm-sp-doc code": { fontFamily: "var(--sp-font-mono)", color: "var(--sp-accent-text)", display: "block", marginBottom: "4px" },
});

const highlight = HighlightStyle.define([
  { tag: [t.keyword, t.controlKeyword, t.definitionKeyword, t.moduleKeyword], color: "#c4a7ff" },
  { tag: [t.string, t.special(t.string)], color: "#9ee6a8" },
  { tag: [t.number, t.bool, t.null], color: "#ffb38a" },
  { tag: [t.comment, t.lineComment, t.blockComment], color: "var(--sp-faint)", fontStyle: "italic" },
  { tag: [t.function(t.variableName), t.function(t.propertyName)], color: "#8ec5ff" },
  { tag: [t.propertyName], color: "#d7dae3" },
  { tag: [t.operator, t.punctuation, t.bracket], color: "var(--sp-muted)" },
  { tag: t.variableName, color: "var(--sp-fg)" },
]);

/** Completes StreamPad globals, namespace members (`window.` → focus, list…) and snippets. */
function apiCompletions(ctx: CompletionContext): CompletionResult | null {
  const word = ctx.matchBefore(/[\w$]+(?:\.[\w$]*)?/);
  if (!word || (word.from === word.to && !ctx.explicit)) return null;
  const text = word.text;
  const dot = text.lastIndexOf(".");
  if (dot >= 0) {
    const ns = text.slice(0, dot);
    return {
      from: word.from + dot + 1,
      options: API.filter((a) => a.name.startsWith(`${ns}.`)).map((a) => ({
        label: a.name.slice(ns.length + 1),
        type: "function",
        detail: a.signature.slice(a.name.length).split(")")[0] + ")",
        info: a.doc,
      })),
      validFor: /^[\w$]*$/,
    };
  }
  const tops = new Map<string, { sig: string; doc: string; ns: boolean }>();
  for (const a of API) {
    const head = a.name.split(".")[0]!;
    if (!tops.has(head)) tops.set(head, { sig: a.name.includes(".") ? "namespace" : a.signature.slice(head.length), doc: a.name.includes(".") ? `${head}.* — ${a.group}` : a.doc, ns: a.name.includes(".") });
  }
  return {
    from: word.from,
    options: [
      ...[...tops].map(([label, v]) => ({ label, type: v.ns ? "namespace" : "function", detail: v.ns ? "" : v.sig.split(")")[0] + ")", info: v.doc, boost: 2 })),
      ...SNIPPETS.map((s) => snippetCompletion(s.code.replace(/\$/g, "\\$"), { label: s.label, detail: s.detail, type: "text", boost: -1 })),
    ],
    validFor: /^[\w$]*$/,
  };
}

const apiHover = hoverTooltip((view, pos) => {
  const line = view.state.doc.lineAt(pos);
  const re = /[\w$]+(?:\.[\w$]+)?/g;
  for (const m of line.text.matchAll(re)) {
    const from = line.from + m.index!;
    const to = from + m[0].length;
    if (pos < from || pos > to) continue;
    const entry = API_BY_NAME.get(m[0]) ?? API_BY_NAME.get(m[0].split(".")[0]!);
    if (!entry) return null;
    return {
      pos: from,
      end: to,
      above: true,
      create() {
        const dom = document.createElement("div");
        dom.className = "cm-sp-doc";
        const sig = document.createElement("code");
        sig.textContent = entry.signature;
        const doc = document.createElement("div");
        doc.textContent = entry.doc;
        dom.append(sig, doc);
        return { dom };
      },
    };
  }
  return null;
});

/** Syntax errors, parsing like the runtime does (function body: top-level `return` is allowed). */
export function syntaxError(source: string): { message: string; line: number; column: number; pos: number } | null {
  try {
    parse(source, { ecmaVersion: "latest", sourceType: "script", allowReturnOutsideFunction: true });
    return null;
  } catch (err) {
    const e = err as { message: string; loc?: { line: number; column: number }; pos?: number };
    return { message: e.message.replace(/\s*\(\d+:\d+\)$/, ""), line: e.loc?.line ?? 1, column: (e.loc?.column ?? 0) + 1, pos: e.pos ?? 0 };
  }
}

const syntaxLint = linter((view): Diagnostic[] => {
  const err = syntaxError(view.state.doc.toString());
  if (!err) return [];
  const from = Math.min(err.pos, view.state.doc.length);
  return [{ from, to: Math.min(from + 1, view.state.doc.length), severity: "error", message: err.message }];
});

export function CodeEditor({ value, onChange, readOnly, api = true, lineNumbers: gutters = false, minHeight, maxHeight, placeholder, className, onView }: CodeEditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const view = useRef<EditorView | null>(null);
  const onChangeRef = useRef(onChange);
  onChangeRef.current = onChange;

  useEffect(() => {
    const extensions: Extension[] = [
      theme,
      syntaxHighlighting(highlight),
      javascript(),
      EditorView.lineWrapping,
      EditorState.readOnly.of(!!readOnly),
      EditorView.editable.of(!readOnly),
      EditorView.updateListener.of((u) => {
        if (u.docChanged) onChangeRef.current?.(u.state.doc.toString());
      }),
    ];
    if (gutters) extensions.push(lineNumbers());
    if (placeholder) extensions.push(placeholderExt(placeholder));
    if (!readOnly) {
      extensions.push(
        history(),
        indentOnInput(),
        bracketMatching(),
        closeBrackets(),
        keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...historyKeymap, ...completionKeymap, indentWithTab]),
      );
      if (api) extensions.push(autocompletion({ override: [apiCompletions], icons: false }), syntaxLint, lintGutter());
    }
    if (api) extensions.push(apiHover);
    const v = new EditorView({ parent: host.current!, state: EditorState.create({ doc: value, extensions }) });
    view.current = v;
    onView?.(v);
    return () => {
      onView?.(null);
      v.destroy();
      view.current = null;
    };
    // The editor is configured once; `value` changes are synced below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [readOnly, api, gutters]);

  useEffect(() => {
    const v = view.current;
    if (v && v.state.doc.toString() !== value) v.dispatch({ changes: { from: 0, to: v.state.doc.length, insert: value } });
  }, [value]);

  return <div ref={host} className={className} style={{ minHeight, maxHeight, overflow: "auto" }} />;
}
