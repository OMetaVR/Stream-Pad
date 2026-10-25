// Cached discovery data for the macro forms (apps, KDE shortcuts, monitors, windows) and derived lists.

import { useEffect, useMemo, useState } from "react";
import { getBackend, type AppEntry, type KdeShortcut } from "@/lib/backend";
import type { Condition, Config, OutputInfo, WindowInfo } from "@/lib/types";
import type { NodeContext } from "./catalog";
import { useEditor } from "@/state/store";

type Loader<T> = () => Promise<T>;

function cached<T>(load: Loader<T>, ttlMs: number) {
  let value: T | null = null;
  let at = 0;
  let pending: Promise<T> | null = null;
  const listeners = new Set<(v: T) => void>();
  const get = (): Promise<T> => {
    if (value && Date.now() - at < ttlMs) return Promise.resolve(value);
    pending ??= load()
      .then((v) => {
        value = v;
        at = Date.now();
        listeners.forEach((l) => l(v));
        return v;
      })
      .finally(() => (pending = null));
    return pending;
  };
  return function useCached(fallback: T): T {
    const [v, setV] = useState<T>(value ?? fallback);
    useEffect(() => {
      listeners.add(setV);
      get().then(setV, () => {});
      return () => void listeners.delete(setV);
    }, []);
    return v;
  };
}

const EMPTY: never[] = [];

export const useApps = cached<AppEntry[]>(() => getBackend().listApps(), 5 * 60_000);
export const useKdeShortcuts = cached<KdeShortcut[]>(() => getBackend().listKdeShortcuts(), 5 * 60_000);
export const useOutputs = cached<OutputInfo[]>(() => getBackend().listOutputs().catch(() => EMPTY), 10_000);
export const useWindows = cached<WindowInfo[]>(() => getBackend().listWindows().catch(() => EMPTY), 2_000);

/** Context the catalog's summaries and default labels need. */
export function useNodeContext(): NodeContext {
  const apps = useApps(EMPTY);
  const profiles = useEditor((s) => s.config?.profiles ?? EMPTY);
  return useMemo(() => ({ apps, profiles }), [apps, profiles]);
}

/** Every variable name used anywhere in the config, for suggestions. */
export function variableNames(config: Config | null): string[] {
  const names = new Set<string>();
  for (const p of config?.profiles ?? []) {
    for (const b of Object.values(p.keys)) {
      const m = b?.macro;
      if (!m) continue;
      if (m.type === "script") {
        for (const x of m.source.matchAll(/vars\.(?:get|set|toggle)\(\s*["'`]([\w.-]+)["'`]/g)) names.add(x[1]!);
        continue;
      }
      for (const n of m.nodes) {
        if ((n.kind === "var.set" || n.kind === "var.toggle") && typeof n.params.name === "string" && n.params.name) names.add(n.params.name);
        const c = n.params.condition as Condition | undefined;
        if (c?.type === "var" && c.name) names.add(c.name);
      }
    }
  }
  return [...names].sort();
}
