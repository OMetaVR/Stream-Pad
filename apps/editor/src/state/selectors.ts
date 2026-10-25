// Small typed hooks over the stores. Components below <LoadGate> may assume the config is loaded.

import { findProfile, globalOf } from "@/lib/model";
import type { Binding, Config, GlobalBinding, KeyId, Profile } from "@/lib/types";
import { useEditor } from "./store";

/** The loaded config. Only call under <LoadGate> (throws otherwise, which is a programming error). */
export function useConfig(): Config {
  const config = useEditor((s) => s.config);
  if (!config) throw new Error("useConfig() used before the config was loaded");
  return config;
}

export function useSelectedProfile(): Profile | undefined {
  return useEditor((s) => (s.config ? findProfile(s.config, s.selection.profileId) : undefined));
}

export function useBinding(profileId: string | null | undefined, keyId: KeyId | null | undefined): Binding | undefined {
  return useEditor((s) => (s.config && keyId ? findProfile(s.config, profileId)?.keys[keyId] : undefined));
}

/** The active global on a key, or null. */
export function useGlobal(keyId: KeyId | null | undefined): GlobalBinding | null {
  return useEditor((s) => (s.config && keyId ? globalOf(s.config.settings, keyId) : null));
}
