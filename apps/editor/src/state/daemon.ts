// Daemon store: connection status + latest DaemonState, live key presses (for press animations), active
// runs and recent log lines. Fed by backend.onDaemonStatus / onDaemonEvent; call `startDaemonStore()` once.
//
//   const running = useDaemon((s) => s.status.kind === "connected");
//   const { pressed, seq } = useKeyPress("KP4");          // seq bumps on every press → retrigger animations
//   useDaemonEvent("trace", (t) => highlight(t.nodeId));  // typed per event kind
//   const lines = useRunLog(runId);

import { useEffect, useRef } from "react";
import { create } from "zustand";
import { useShallow } from "zustand/react/shallow";
import { getBackend, type DaemonStatus } from "@/lib/backend";
import type { DaemonEvent, DaemonState, KeyId } from "@/lib/types";

export type DaemonEventKind = DaemonEvent["event"];
export type DaemonEventData<K extends DaemonEventKind> = Extract<DaemonEvent, { event: K }>["data"];

export interface RunInfo {
  runId: number;
  key: KeyId | null;
  startedAt: number;
}

export interface LogEntry {
  /** Monotonic id (React key). */
  id: number;
  runId: number | null;
  level: "info" | "warn" | "error";
  message: string;
  at: number;
}

export interface DaemonStoreState {
  status: DaemonStatus;
  /** Latest snapshot while connected, else null. */
  state: DaemonState | null;
  /** Keys currently held → the press sequence number of that press. */
  held: Partial<Record<KeyId, number>>;
  /** Last press sequence number per key (kept after release). */
  pressSeq: Partial<Record<KeyId, number>>;
  /** Runs that have started and not yet finished/failed/been cancelled. */
  runs: Record<number, RunInfo>;
  /** Most recent log lines (script `log()`, run errors), oldest first. */
  logs: LogEntry[];
}

const LOG_LIMIT = 400;

export const useDaemon = create<DaemonStoreState>()(() => ({
  status: { kind: "connecting" },
  state: null,
  held: {},
  pressSeq: {},
  runs: {},
  logs: [],
}));

type AnyListener = (data: unknown) => void;
const listeners = new Map<DaemonEventKind, Set<AnyListener>>();
let seq = 0;
let logId = 0;
let started = false;

/** Subscribe to one daemon event kind outside React. Returns the unsubscribe function. */
export function onDaemonEvent<K extends DaemonEventKind>(kind: K, cb: (data: DaemonEventData<K>) => void): () => void {
  const set = listeners.get(kind) ?? new Set();
  listeners.set(kind, set);
  set.add(cb as AnyListener);
  return () => set.delete(cb as AnyListener);
}

/** Hook form of `onDaemonEvent`; the callback may change between renders without resubscribing. */
export function useDaemonEvent<K extends DaemonEventKind>(kind: K, cb: (data: DaemonEventData<K>) => void): void {
  const ref = useRef(cb);
  ref.current = cb;
  useEffect(() => onDaemonEvent(kind, (d) => ref.current(d)), [kind]);
}

export function useKeyPress(key: KeyId): { pressed: boolean; seq: number } {
  return useDaemon(useShallow((s) => ({ pressed: s.held[key] !== undefined, seq: s.pressSeq[key] ?? 0 })));
}

/** Log lines of one run (or all lines when `runId` is null). */
export function useRunLog(runId: number | null): LogEntry[] {
  return useDaemon(useShallow((s) => (runId === null ? s.logs : s.logs.filter((l) => l.runId === runId))));
}

export function clearLogs(): void {
  useDaemon.setState({ logs: [] });
}

function appendLog(entry: Omit<LogEntry, "id" | "at">) {
  useDaemon.setState((s) => ({ logs: [...s.logs, { ...entry, id: ++logId, at: Date.now() }].slice(-LOG_LIMIT) }));
}

function handle(e: DaemonEvent) {
  switch (e.event) {
    case "key": {
      const { key, pressed } = e.data;
      useDaemon.setState((s) => {
        const held = { ...s.held };
        if (pressed) {
          held[key] = ++seq;
          return { held, pressSeq: { ...s.pressSeq, [key]: seq } };
        }
        delete held[key];
        return { held };
      });
      break;
    }
    case "run": {
      const { runId, key, status, error } = e.data;
      useDaemon.setState((s) => {
        const runs = { ...s.runs };
        if (status === "started") runs[runId] = { runId, key, startedAt: Date.now() };
        else delete runs[runId];
        return { runs };
      });
      if (status === "failed" && error) appendLog({ runId, level: "error", message: error });
      break;
    }
    case "log":
      appendLog({ runId: e.data.runId, level: e.data.level, message: e.data.message });
      break;
    default:
      // `state` reaches the store through the status channel (backends fold it into DaemonStatus).
      break;
  }
  listeners.get(e.event)?.forEach((l) => {
    try {
      l(e.data);
    } catch (err) {
      console.error(`daemon "${e.event}" listener failed`, err);
    }
  });
}

function applyStatus(status: DaemonStatus) {
  if (status.kind === "connected") {
    useDaemon.setState({ status, state: status.state });
  } else {
    // Without a connection nothing is held or running any more.
    useDaemon.setState({ status, state: null, held: {}, runs: {} });
  }
}

/** Wire the store to the backend. Idempotent. */
export function startDaemonStore(): void {
  if (started) return;
  started = true;
  const backend = getBackend();
  applyStatus(backend.daemonStatus());
  backend.onDaemonStatus(applyStatus);
  backend.onDaemonEvent(handle);
}
