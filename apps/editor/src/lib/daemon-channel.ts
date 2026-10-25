// Status + event fan-out shared by both backends. `state` events are folded into the connected status, so
// `daemonStatus()` always carries the latest DaemonState and status listeners see every state change.

import type { DaemonStatus } from "./backend";
import type { DaemonEvent } from "./types";

type Listener<T> = (value: T) => void;

function notify<T>(listeners: Set<Listener<T>>, value: T) {
  for (const l of [...listeners]) {
    try {
      l(value);
    } catch (e) {
      console.error("daemon listener failed", e);
    }
  }
}

export class DaemonChannel {
  private status: DaemonStatus = { kind: "connecting" };
  private readonly statusListeners = new Set<Listener<DaemonStatus>>();
  private readonly eventListeners = new Set<Listener<DaemonEvent>>();

  getStatus(): DaemonStatus {
    return this.status;
  }

  setStatus(next: DaemonStatus): void {
    this.status = next;
    notify(this.statusListeners, next);
  }

  pushEvent(event: DaemonEvent): void {
    if (event.event === "state" && this.status.kind === "connected") {
      this.setStatus({ kind: "connected", state: event.data });
    }
    notify(this.eventListeners, event);
  }

  onStatus(cb: Listener<DaemonStatus>): () => void {
    this.statusListeners.add(cb);
    return () => this.statusListeners.delete(cb);
  }

  onEvent(cb: Listener<DaemonEvent>): () => void {
    this.eventListeners.add(cb);
    return () => this.eventListeners.delete(cb);
  }
}

/** Tauri rejects with the command's error string; normalise everything to `Error`. */
export function toError(e: unknown): Error {
  if (e instanceof Error) return e;
  if (typeof e === "string") return new Error(e);
  if (e && typeof e === "object" && "message" in e && typeof e.message === "string") return new Error(e.message);
  return new Error(String(e));
}
