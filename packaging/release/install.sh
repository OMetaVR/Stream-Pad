#!/usr/bin/env bash
# StreamPad release installer: installs the prebuilt app for the current user (default ~/.local),
# enables its background service and checks that your keyboards are accessible.
#   ./install.sh [--prefix DIR] [--no-start]
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
prefix=$HOME/.local
start=yes
while [[ $# -gt 0 ]]; do
    case $1 in
        --prefix) prefix=${2:?--prefix needs a directory}; shift 2 ;;
        --no-start) start=no; shift ;;
        -h|--help) sed -n '2,4p' "$0"; exit 0 ;;
        *) echo "unknown option $1" >&2; exit 2 ;;
    esac
done
[[ $EUID -ne 0 ]] || { echo "Run this as your normal user, not root (it installs to your home and enables a user service)." >&2; exit 1; }

payload=$here/payload
ver=$(cat "$here/VERSION")
echo "Installing StreamPad $ver to $prefix"

# 1. Runtime libraries (the binaries link against your distro's Qt 6, layer-shell-qt and WebKitGTK).
missing=$(for b in "$payload"/bin/*; do ldd "$b" 2>/dev/null | awk '/not found/ {print $1}'; done | sort -u)
if [[ -n $missing ]]; then
    echo
    echo "These libraries are missing:"
    printf '    %s\n' $missing
    if command -v pacman >/dev/null; then
        echo "Install them with:"
        echo "    sudo pacman -S --needed gtk3 webkit2gtk-4.1 qt6-base qt6-declarative qt6-svg layer-shell-qt kwindowsystem"
    else
        echo "This build targets Arch-based distros. On other distros, build from source instead: https://github.com/OMetaVR/Stream-Pad"
    fi
    exit 1
fi

# 2. Files.
if systemctl --user is-active --quiet streampad 2>/dev/null; then
    systemctl --user stop streampad
    restart=yes
fi
install -d "$prefix/bin"
install -m755 "$payload"/bin/* "$prefix/bin/"
# PREFIX/share/streampad doubles as your data dir for a ~/.local install (uploaded covers in covers/, your
# icons in icons/), so replace only what the release ships — never the directory itself.
rm -rf "$prefix/share/streampad/icons/lucide" "$prefix/share/streampad/udev"
mkdir -p "$prefix/share/streampad"
cp -r "$payload/share/streampad/." "$prefix/share/streampad/"
install -Dm644 "$payload/share/icons/hicolor/scalable/apps/io.github.ometavr.streampad.svg" \
    "$prefix/share/icons/hicolor/scalable/apps/io.github.ometavr.streampad.svg"
desktop=$prefix/share/applications/io.github.ometavr.streampad.desktop
install -Dm644 "$payload/share/applications/io.github.ometavr.streampad.desktop" "$desktop"
sed -i "s|/__PREFIX__|$prefix|g" "$desktop"
command -v update-desktop-database >/dev/null && update-desktop-database -q "$prefix/share/applications" || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$prefix/share/icons/hicolor" 2>/dev/null || true

unit_dir=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
install -d "$unit_dir"
sed "s|/__PREFIX__|$prefix|g" "$payload/lib/systemd/user/streampad.service" > "$unit_dir/streampad.service"
install -m755 "$here/uninstall.sh" "$prefix/share/streampad/uninstall.sh"

# 3. Service.
systemctl --user daemon-reload
if systemctl --user enable streampad.service >/dev/null 2>&1; then
    echo "Enabled the StreamPad service (starts at login)."
else
    echo "Couldn't enable the service; start it with: systemctl --user enable --now streampad"
fi

# 4. Keyboard access.
access=ok
"$here/check-access.sh" "$prefix/share/streampad/udev/70-streampad.rules" || access=missing

if [[ $access == ok && ( $start == yes || ${restart:-no} == yes ) ]]; then
    systemctl --user start streampad && echo "StreamPad is running."
fi

cat <<MSG

Open the editor from your app launcher ("StreamPad") or run: $prefix/bin/streampad
Scroll Lock toggles macro mode; Numpad Enter shows the pad, + / − switch profiles.
Uninstall any time with: $prefix/share/streampad/uninstall.sh
MSG
[[ :$PATH: == *:"$prefix/bin":* ]] || echo "Note: $prefix/bin is not on your PATH (the launcher entry works regardless)."
