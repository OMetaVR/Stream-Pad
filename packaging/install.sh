#!/usr/bin/env bash
# Installs a built StreamPad (see `make`) for the current user (default PREFIX=~/.local).
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
PREFIX=${PREFIX:-$HOME/.local}
TARGET=${TARGET:-$root/target/release-all}
BUILD=${BUILD:-$root/build}
[[ $BUILD = /* ]] || BUILD=$root/$BUILD
system=$([[ $PREFIX == "$HOME"* ]] && echo no || echo yes)

bin_daemon=$TARGET/release/streampadd
bin_editor=$TARGET/release/streampad
bin_overlay=$BUILD/overlay/streampad-overlay
for f in "$bin_daemon" "$bin_editor" "$bin_overlay"; do
    [[ -x $f ]] || { echo "missing $f — run 'make' first" >&2; exit 1; }
done

echo "Installing StreamPad to $PREFIX"
install -Dm755 "$bin_daemon"  "$PREFIX/bin/streampadd"
install -Dm755 "$bin_overlay" "$PREFIX/bin/streampad-overlay"
install -Dm755 "$bin_editor"  "$PREFIX/bin/streampad"

share=$PREFIX/share/streampad
rm -rf "$share/icons/lucide"
mkdir -p "$share/icons/lucide"
cp "$BUILD"/share/streampad/icons/lucide/* "$share/icons/lucide/"

install -Dm644 "$root/packaging/applications/io.github.ometavr.streampad.desktop" "$PREFIX/share/applications/io.github.ometavr.streampad.desktop"
sed -i "s|^Exec=streampad$|Exec=$PREFIX/bin/streampad|" "$PREFIX/share/applications/io.github.ometavr.streampad.desktop"
install -Dm644 "$root/packaging/icons/streampad.svg" "$PREFIX/share/icons/hicolor/scalable/apps/io.github.ometavr.streampad.svg"
command -v update-desktop-database >/dev/null && update-desktop-database -q "$PREFIX/share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$PREFIX/share/icons/hicolor" 2>/dev/null || true

if [[ $system == yes ]]; then
    unit_dir=$PREFIX/lib/systemd/user
else
    unit_dir=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
fi
mkdir -p "$unit_dir"
sed "s|^ExecStart=.*|ExecStart=$PREFIX/bin/streampadd|" "$root/packaging/systemd/streampad.service" > "$unit_dir/streampad.service"
if [[ $system == no ]] && command -v systemctl >/dev/null; then
    systemctl --user daemon-reload
    systemctl --user enable streampad.service >/dev/null 2>&1 && echo "Enabled streampad.service (starts at login)."
fi

# Input permissions: the daemon needs to read keyboards and create virtual devices.
if [[ ! -r /dev/uinput || ! -w /dev/uinput ]] || ! ls /dev/input/event* >/dev/null 2>&1 || [[ ! -r $(ls /dev/input/event* | head -1) ]]; then
    cat <<MSG

StreamPad can't access your keyboards yet. Either add yourself to the input group:
    sudo usermod -aG input "$USER"     (then log out and back in)
or install the udev rule that grants the logged-in user access:
    sudo install -m644 "$root/packaging/udev/70-streampad.rules" /etc/udev/rules.d/
    sudo udevadm control --reload && sudo udevadm trigger
MSG
fi

cat <<MSG

Done. Start it now with:   systemctl --user start streampad
Open the editor:           streampad   (or "StreamPad" in your app launcher)
Scroll Lock toggles macro mode; Numpad Enter shows the pad, + / − switch profiles.
MSG
