#!/usr/bin/env bash
# Regenerates audio.json / apps.json from REAL daemon state: runs streampadd (no input, no overlay) in a
# throw-away directory, gives Audio's key 9 a synthwave cover, and saves `ctl getState` for both profiles.
# Never touches the real ~/.config/streampad or the running service. Output is made portable: lucide
# icons become "lucide:<name>", the cover becomes a path relative to this directory.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
HERE="$ROOT/promo/overlay"
SPD="$ROOT/target/release-all/release/streampadd"
[ -x "$SPD" ] || CARGO_TARGET_DIR="$ROOT/target/release-all" nice -n 10 cargo build --release -p streampadd

T="$(mktemp -d /tmp/sp-promo-states.XXXXXX)"
mkdir -p "$T"/{config,data/covers,state,run}
export STREAMPAD_CONFIG_DIR="$T/config" STREAMPAD_DATA_DIR="$T/data" STREAMPAD_STATE_DIR="$T/state" \
       STREAMPAD_RUNTIME_DIR="$T/run" STREAMPAD_SHARE_DIR="$ROOT/build/share/streampad"
"$SPD" --no-input --no-overlay >"$T/daemon.log" 2>&1 &
PID=$!
trap 'kill $PID 2>/dev/null || true; wait $PID 2>/dev/null || true; rm -rf "$T"' EXIT
for _ in $(seq 50); do [ -S "$T/run/daemon.sock" ] && [ -f "$T/config/config.json" ] && break; sleep 0.1; done

cp "$ROOT/apps/editor/public/mock/covers/synthwave.png" "$T/data/covers/"
python3 -I - "$T/config/config.json" <<'PY'
import json, os, sys
path = sys.argv[1]
cfg = json.load(open(path))
audio = next(p for p in cfg["profiles"] if p["name"] == "Audio")
audio["keys"]["KP9"] = {
    "label": "Playlist", "icon": "lucide:music", "cover": "covers/synthwave.png", "coverCrop": None, "color": None,
    "macro": {"type": "graph",
              "nodes": [{"id": "press", "kind": "trigger.press", "params": {}, "position": {"x": 0.0, "y": 0.0}},
                        {"id": "action", "kind": "app.launch", "params": {"app": "spotify", "focusIfRunning": True},
                         "position": {"x": 240.0, "y": 0.0}}],
              "edges": [{"id": "e_press_out", "source": "press", "sourcePort": "out", "target": "action"}]},
    "repeat": False, "concurrency": "ignore"}
json.dump(cfg, open(path + ".tmp", "w"), indent=2)
os.replace(path + ".tmp", path)  # atomic, like the editor
PY
for _ in $(seq 50); do
  "$SPD" ctl getState 2>/dev/null | grep -q '"cover": ".*synthwave' && break; sleep 0.1
done
"$SPD" ctl getState >"$T/audio.json"
"$SPD" ctl setProfile '{"index":1}' >/dev/null
"$SPD" ctl getState >"$T/apps.json"

cp "$T/data/covers/synthwave.png" "$HERE/covers/synthwave.png"
python3 -I - "$T" "$HERE" "$ROOT/build/share/streampad/icons/lucide/" <<'PY'
import json, sys
tmp, here, lucide = sys.argv[1:]
def portable(path):
    if path and path.startswith(lucide) and path.endswith(".svg"):
        return "lucide:" + path[len(lucide):-4]
    return path
for name in ("audio", "apps"):
    st = json.load(open(f"{tmp}/{name}.json"))
    st["overlay"]["visible"] = False
    st["overlay"]["output"] = None
    st["profile"]["icon"] = portable(st["profile"].get("icon"))
    for k in st["profile"]["keys"].values():
        k["icon"] = portable(k.get("icon"))
        if k.get("cover"):
            k["cover"] = "covers/" + k["cover"].rsplit("/", 1)[1]
    for p in st["profiles"]:
        p["icon"] = portable(p.get("icon"))
    json.dump(st, open(f"{here}/{name}.json", "w"), indent=2, sort_keys=True)
    open(f"{here}/{name}.json", "a").write("\n")
PY
echo "wrote $HERE/audio.json and apps.json"
