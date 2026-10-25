#!/usr/bin/env bash
# Renders every fixture offscreen into PNGs:  test/render_fixtures.sh [out-dir]
# With BACKDROP=1 each shot is also composited over a wallpaper-like gradient (<name>-backdrop.png),
# which makes transparency, shadows and fillets easier to judge (needs ImageMagick).
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
bin=${OVERLAY_BIN:-$here/../build/streampad-overlay}
out=${1:-/tmp/streampad-overlay-shots}
repo=$(cd "$here/../../.." && pwd)
mkdir -p "$out"

export QT_QPA_PLATFORM=offscreen
export STREAMPAD_SHARE_DIR=${STREAMPAD_SHARE_DIR:-$repo/build/share/streampad}

shot() {
    local name=$1 fixture=$2
    shift 2
    timeout 30 "$bin" --mock "$here/fixtures/$fixture.json" --screenshot "$out/$name.png" "$@" >/dev/null
    if [[ ${BACKDROP:-0} == 1 ]]; then
        local size
        size=$(magick identify -format '%wx%h' "$out/$name.png")
        magick -size "$size" gradient:'#5b6f96-#1b2030' "$out/$name.png" -compose over -composite \
            "$out/$name-backdrop.png"
    fi
    echo "$out/$name.png"
}

for fixture in "$here"/fixtures/*.json; do
    name=$(basename "$fixture" .json)
    [[ $name == hidden ]] && continue
    shot "$name" "$name"
done
shot pressed dark --mock-key KP5 --mock-key KPPlus --mock-key KP7
shot toasts dark --mock-toast "Game mode: ON" --mock-toast "success:Saved to clipboard" \
    --mock-toast "error:Script failed: window.focus timed out"
shot toasts-left edge-left-center --mock-toast "Desktop 2" --mock-toast "error:Not supported on this desktop"
shot slide-mid dark --after-ms 90
