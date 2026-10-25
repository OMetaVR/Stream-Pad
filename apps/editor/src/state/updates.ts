// Update checks against GitHub releases: the latest result, install progress, and the once-a-day automatic
// check (an editor preference, kept in localStorage).

import { create } from "zustand";
import { getBackend, type UpdateInfo, type UpdateProgress } from "@/lib/backend";

const AUTO_KEY = "streampad.updates.auto";
const LAST_KEY = "streampad.updates.lastCheck";
const INFO_KEY = "streampad.updates.lastInfo";
const DAY = 86_400_000;

type InstallState =
  | { kind: "idle" }
  | { kind: "running"; progress: UpdateProgress | null }
  | { kind: "installed"; version: string }
  | { kind: "error"; message: string };

interface UpdatesState {
  info: UpdateInfo | null;
  checking: boolean;
  checkError: string | null;
  checkedAt: number | null;
  auto: boolean;
  install: InstallState;
}

function cachedInfo(): UpdateInfo | null {
  try {
    const info = JSON.parse(localStorage.getItem(INFO_KEY) ?? "null") as UpdateInfo | null;
    // Stale once this copy has been updated (or downgraded) since.
    return info && info.current === getBackend().appVersion() ? info : null;
  } catch {
    return null;
  }
}

export const useUpdates = create<UpdatesState>()(() => ({
  info: null,
  checking: false,
  checkError: null,
  checkedAt: Number(localStorage.getItem(LAST_KEY)) || null,
  auto: localStorage.getItem(AUTO_KEY) !== "0",
  install: { kind: "idle" },
}));

const set = useUpdates.setState;

export async function checkForUpdate(): Promise<UpdateInfo | null> {
  if (useUpdates.getState().checking) return null;
  set({ checking: true, checkError: null });
  try {
    const info = await getBackend().checkForUpdate();
    const now = Date.now();
    localStorage.setItem(LAST_KEY, String(now));
    localStorage.setItem(INFO_KEY, JSON.stringify(info));
    set({ info, checkedAt: now });
    return info;
  } catch (e) {
    set({ checkError: e instanceof Error ? e.message : String(e) });
    return null;
  } finally {
    set({ checking: false });
  }
}

export function setAutoCheck(on: boolean): void {
  localStorage.setItem(AUTO_KEY, on ? "1" : "0");
  set({ auto: on });
}

export async function installUpdate(): Promise<void> {
  set({ install: { kind: "running", progress: null } });
  try {
    const version = await getBackend().installUpdate((progress) => set({ install: { kind: "running", progress } }));
    set({ install: { kind: "installed", version } });
  } catch (e) {
    set({ install: { kind: "error", message: e instanceof Error ? e.message : String(e) } });
  }
}

/** Automatic check shortly after launch, at most once a day. Resolves with the info when an update is out. */
export async function autoCheck(): Promise<UpdateInfo | null> {
  const { auto, checkedAt } = useUpdates.getState();
  if (!auto || getBackend().isMock) return null;
  if (checkedAt && Date.now() - checkedAt < DAY) {
    const info = cachedInfo();
    if (info) set({ info });
    return null; // already announced on the launch that checked
  }
  const info = await checkForUpdate();
  return info?.available ? info : null;
}
