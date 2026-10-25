// Autosave: every config change is written 400 ms after the last edit (atomic write in the backend; the
// daemon hot-reloads). Saves never overlap; a change made during a save is written right after it.
// Status for the header indicator lives in `useEditor((s) => s.save)`.

import { getBackend } from "@/lib/backend";
import type { Config } from "@/lib/types";
import { setSaveState, useEditor } from "./store";

const DEBOUNCE_MS = 400;

let lastSaved: Config | null = null;
let timer: ReturnType<typeof setTimeout> | null = null;
let inFlight: Promise<void> | null = null;
let started = false;

function dirty(): boolean {
  const { config, load } = useEditor.getState();
  return load.kind === "ready" && config !== null && config !== lastSaved;
}

async function saveNow(): Promise<void> {
  if (timer) {
    clearTimeout(timer);
    timer = null;
  }
  // Another caller may start a save while we wait, so wait until none is running.
  while (inFlight) await inFlight;
  const config = useEditor.getState().config;
  if (!config || !dirty()) return;
  const prevSavedAt = useEditor.getState().save.savedAt;
  setSaveState({ status: "saving", error: null, savedAt: prevSavedAt });
  inFlight = getBackend()
    .saveConfig(config)
    .then(
      () => {
        lastSaved = config;
        setSaveState({ status: "saved", error: null, savedAt: Date.now() });
      },
      (e: unknown) => {
        setSaveState({ status: "error", error: e instanceof Error ? e.message : String(e), savedAt: prevSavedAt });
      },
    )
    .finally(() => {
      inFlight = null;
    });
  await inFlight;
}

function schedule() {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => void saveNow(), DEBOUNCE_MS);
}

/** Start watching the store. Idempotent. */
export function startAutosave(): void {
  if (started) return;
  started = true;
  useEditor.subscribe((s, prev) => {
    if (s.load.kind === "ready" && prev.load.kind !== "ready") {
      lastSaved = s.config; // freshly loaded = already on disk
      return;
    }
    if (s.config !== prev.config && dirty()) schedule();
  });
}

/** Write pending changes immediately (window close, before quitting). */
export function flushSave(): Promise<void> {
  return dirty() ? saveNow() : (inFlight ?? Promise.resolve());
}

/** Retry after a failed save (the indicator's "Retry"). */
export function retrySave(): Promise<void> {
  return saveNow();
}

export function hasUnsavedChanges(): boolean {
  return dirty() || inFlight !== null;
}
