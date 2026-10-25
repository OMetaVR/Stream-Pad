#!/usr/bin/env bash
# Builds the release artifacts into dist/ from an already-built tree (run `make` first):
#   dist/streampad-<ver>-linux-x86_64.tar.gz    prebuilt app + ./install.sh (Arch-based distros)
#   dist/SHA256SUMS
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
ver=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)
name=streampad-$ver-linux-$(uname -m)
dist=$root/dist
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

"$root/packaging/stage.sh" "$work/stage" /__PREFIX__
pkg=$work/$name
mkdir -p "$pkg"
mv "$work/stage/__PREFIX__" "$pkg/payload"
install -m755 "$root/packaging/release/install.sh" "$root/packaging/release/uninstall.sh" "$root/packaging/check-access.sh" "$pkg/"
echo "$ver" > "$pkg/VERSION"
cat > "$pkg/README.txt" <<TXT
StreamPad $ver — numpad → macropad for Linux (KDE Plasma 6 / Wayland first)
https://github.com/OMetaVR/Stream-Pad

Install:    ./install.sh          (as your normal user; installs to ~/.local and starts the service)
Uninstall:  ./uninstall.sh

Needs Qt 6 (base, declarative, svg), layer-shell-qt, KWindowSystem, GTK 3 and WebKitGTK 4.1 — on Arch:
  sudo pacman -S --needed gtk3 webkit2gtk-4.1 qt6-base qt6-declarative qt6-svg layer-shell-qt kwindowsystem
Built on Arch Linux; on other distros build from source (see the repository README).
TXT

mkdir -p "$dist"
tar -C "$work" --owner=0 --group=0 -czf "$dist/$name.tar.gz" "$name"
(cd "$dist" && shopt -s nullglob && sha256sum -- *.tar.gz *.pkg.tar.zst > SHA256SUMS)
ls -la "$dist"
