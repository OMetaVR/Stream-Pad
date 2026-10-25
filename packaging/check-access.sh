#!/usr/bin/env bash
# Explains how to grant keyboard access when the current user can't read keyboards or create virtual
# devices (StreamPad needs both). Exit 0 = access is fine, 1 = action needed.
#   usage: check-access.sh [path/to/70-streampad.rules]
rules=${1:-/usr/share/streampad/udev/70-streampad.rules}

ok=yes
[[ -r /dev/uinput && -w /dev/uinput ]] || ok=no
for dev in /dev/input/event*; do
    [[ -e $dev ]] || continue
    [[ -r $dev ]] || ok=no
    break
done
[[ $ok == yes ]] && exit 0

cat <<MSG

StreamPad can't access your keyboards yet. Either add yourself to the input group:
    sudo usermod -aG input "$USER"     (then log out and back in)
or install the udev rule that grants the logged-in user access (no re-login needed):
    sudo install -m644 "$rules" /etc/udev/rules.d/
    sudo udevadm control --reload && sudo udevadm trigger
MSG
exit 1
