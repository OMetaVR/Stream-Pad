#!/usr/bin/env bash
# Lays out a built StreamPad (see `make`) under DESTDIR/PREFIX, the way a package installs it:
#   bin/streampadd  bin/streampad-overlay  bin/streampad
#   share/streampad/icons/lucide/*.svg     share/streampad/udev/70-streampad.rules
#   share/applications/…desktop            share/icons/hicolor/scalable/apps/…svg
#   lib/systemd/user/streampad.service     share/licenses/streampad/*
# Used by `make install` (packaging/install.sh), the Arch package and the release tarball.
#   usage: packaging/stage.sh DESTDIR PREFIX      e.g. packaging/stage.sh "$pkgdir" /usr
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
destdir=${1?usage: stage.sh DESTDIR PREFIX}
prefix=${2:?usage: stage.sh DESTDIR PREFIX}
target=${TARGET:-$root/target/release-all}
build=${BUILD:-$root/build}
[[ $build = /* ]] || build=$root/$build

bin_daemon=$target/release/streampadd
bin_editor=$target/release/streampad
bin_overlay=$build/overlay/streampad-overlay
for f in "$bin_daemon" "$bin_editor" "$bin_overlay"; do
    [[ -x $f ]] || { echo "stage.sh: missing $f — run 'make' first" >&2; exit 1; }
done
[[ -d $build/share/streampad/icons/lucide ]] || { echo "stage.sh: missing icons — run 'make share'" >&2; exit 1; }

out=$destdir$prefix
install -Dm755 "$bin_daemon"  "$out/bin/streampadd"
install -Dm755 "$bin_overlay" "$out/bin/streampad-overlay"
install -Dm755 "$bin_editor"  "$out/bin/streampad"

rm -rf "$out/share/streampad/icons/lucide"
install -d "$out/share/streampad/icons/lucide"
install -m644 "$build"/share/streampad/icons/lucide/*.svg "$out/share/streampad/icons/lucide/"
install -Dm644 "$root/packaging/udev/70-streampad.rules" "$out/share/streampad/udev/70-streampad.rules"

desktop=$out/share/applications/io.github.ometavr.streampad.desktop
install -Dm644 "$root/packaging/applications/io.github.ometavr.streampad.desktop" "$desktop"
# Outside /usr the launcher may not have PREFIX/bin on its PATH.
[[ $prefix == /usr ]] || sed -i "s|^Exec=streampad$|Exec=$prefix/bin/streampad|" "$desktop"
install -Dm644 "$root/packaging/icons/streampad.svg" "$out/share/icons/hicolor/scalable/apps/io.github.ometavr.streampad.svg"

install -d "$out/lib/systemd/user"
sed "s|^ExecStart=.*|ExecStart=$prefix/bin/streampadd|" "$root/packaging/systemd/streampad.service" > "$out/lib/systemd/user/streampad.service"
chmod 644 "$out/lib/systemd/user/streampad.service"

lic=$out/share/licenses/streampad
install -Dm644 "$root/apps/overlay/fonts/OFL.txt" "$lic/Inter-OFL.txt"
[[ -f $build/share/streampad/icons/lucide/LICENSE ]] && install -Dm644 "$build/share/streampad/icons/lucide/LICENSE" "$lic/lucide-LICENSE"
true
