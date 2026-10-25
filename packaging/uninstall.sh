#!/usr/bin/env bash
# Removes what install.sh installed. Your config (~/.config/streampad) and covers (~/.local/share/streampad) stay.
set -euo pipefail
PREFIX=${PREFIX:-$HOME/.local}
if command -v systemctl >/dev/null; then
    systemctl --user disable --now streampad.service >/dev/null 2>&1 || true
fi
rm -f "${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user/streampad.service" "$PREFIX/lib/systemd/user/streampad.service"
command -v systemctl >/dev/null && systemctl --user daemon-reload || true
rm -f "$PREFIX/bin/streampadd" "$PREFIX/bin/streampad-overlay" "$PREFIX/bin/streampad"
rm -rf "$PREFIX/share/streampad"
rm -f "$PREFIX/share/applications/io.github.ometavr.streampad.desktop" "$PREFIX/share/icons/hicolor/scalable/apps/io.github.ometavr.streampad.svg"
echo "StreamPad removed. Your profiles in ~/.config/streampad were kept."
