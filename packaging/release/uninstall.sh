#!/usr/bin/env bash
# Removes StreamPad installed by the release installer. Your profiles (~/.config/streampad) and covers
# (~/.local/share/streampad) are kept.   ./uninstall.sh [--prefix DIR]
set -euo pipefail
prefix=$HOME/.local
[[ ${1:-} == --prefix ]] && prefix=${2:?--prefix needs a directory}
systemctl --user disable --now streampad.service >/dev/null 2>&1 || true
rm -f "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/streampad.service"
systemctl --user daemon-reload 2>/dev/null || true
rm -f "$prefix/bin/streampadd" "$prefix/bin/streampad-overlay" "$prefix/bin/streampad"
rm -f "$prefix/share/applications/io.github.ometavr.streampad.desktop" \
      "$prefix/share/icons/hicolor/scalable/apps/io.github.ometavr.streampad.svg"
rm -rf "$prefix/share/streampad"
echo "StreamPad removed. Your profiles in ~/.config/streampad were kept."
