#!/usr/bin/env bash
# Installs a built StreamPad (see `make`) for the current user (default PREFIX=~/.local) and enables the
# background service. A system prefix (e.g. PREFIX=/usr/local, run as root) installs files only.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
PREFIX=${PREFIX:-$HOME/.local}
system=$([[ $PREFIX == "$HOME"* ]] && echo no || echo yes)

echo "Installing StreamPad to $PREFIX"
"$root/packaging/stage.sh" "" "$PREFIX"

command -v update-desktop-database >/dev/null && update-desktop-database -q "$PREFIX/share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true

if [[ $system == no ]]; then
    # systemd's user search path has ~/.config/systemd/user, not PREFIX/lib/systemd/user.
    unit_dir=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
    install -Dm644 "$PREFIX/lib/systemd/user/streampad.service" "$unit_dir/streampad.service"
    rm -rf "$PREFIX/lib/systemd/user/streampad.service"
    rmdir -p "$PREFIX/lib/systemd/user" 2>/dev/null || true
    if command -v systemctl >/dev/null; then
        systemctl --user daemon-reload
        systemctl --user enable streampad.service >/dev/null 2>&1 && echo "Enabled streampad.service (starts at login)."
    fi
fi

"$root/packaging/check-access.sh" "$PREFIX/share/streampad/udev/70-streampad.rules" || true

cat <<MSG

Done. Start it now with:   systemctl --user start streampad
Open the editor:           streampad   (or "StreamPad" in your app launcher)
Scroll Lock toggles macro mode; Numpad Enter shows the pad, + / − switch profiles.
MSG
