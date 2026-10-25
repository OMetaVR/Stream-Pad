// Keycap face data for the intro and the placeholder overlay, from the same sample config the editor uses.

import { GLOBAL_ACTION_INFO, globalOf } from "@/lib/model";
import { sampleConfig } from "@/lib/mock/sample-config";
import { KEY_IDS, type CropRect, type KeyId } from "@/lib/types";

export interface Face {
  label: string | null;
  icon: string | null;
  cover: string | null;
  coverCrop: CropRect | null;
  global: boolean;
  empty: boolean;
}

const config = sampleConfig();

export function profileFaces(name: string): Record<KeyId, Face> {
  const profile = config.profiles.find((p) => p.name === name)!;
  const out = {} as Record<KeyId, Face>;
  for (const k of KEY_IDS) {
    const g = globalOf(config.settings, k);
    const b = profile.keys[k];
    // Animated covers would play on the wall clock — the stand-in overlay shows none.
    const cover = !g && b?.cover && !b.cover.endsWith(".gif") ? (b.cover.startsWith("/") ? b.cover : `/mock/${b.cover}`) : null;
    out[k] = {
      label: g ? GLOBAL_ACTION_INFO[g.tap].label : (b?.label ?? null),
      icon: g ? `lucide:${GLOBAL_ACTION_INFO[g.tap].icon}` : (b?.icon ?? null),
      cover,
      coverCrop: b?.coverCrop ?? null,
      global: !!g,
      empty: !g && !b?.macro && !b?.label && !b?.icon && !cover,
    };
  }
  return out;
}

export const PROFILE_COLORS = Object.fromEntries(config.profiles.map((p) => [p.name, p.color ?? "#8b7cf6"]));
