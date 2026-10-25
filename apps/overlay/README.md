# streampad-overlay

The on-screen pad (Qt 6 / QML, layer-shell on KDE Wayland). Built by the top-level `make`, or on its own:
`cmake -S apps/overlay -B apps/overlay/build -G Ninja && cmake --build apps/overlay/build`.

It connects to the daemon socket; for development it can render a DaemonState file instead:

```sh
QT_QPA_PLATFORM=offscreen streampad-overlay --mock state.json --screenshot out.png [--after-ms N] [--mock-key KP5] [--mock-toast "success:Saved"]
```

## Frame capture (`--capture`)

Renders a scripted run of daemon events to a PNG sequence, with every animation on a fixed-step virtual
clock (no wall-clock dependence: the same script gives byte-identical frames). Used for the README promo.

```sh
QT_QPA_PLATFORM=offscreen streampad-overlay --capture script.json --out frames/
```

`script.json` (paths are relative to the script):

```json
{ "fps": 30, "frames": 126, "state": "audio.json",
  "events": [
    { "frame": 9,  "type": "visible", "value": true },
    { "frame": 39, "type": "key", "key": "KP5", "pressed": true },
    { "frame": 39, "type": "toast", "text": "Play / Pause", "kind": "info", "durationMs": 1600 },
    { "frame": 63, "type": "profile", "state": "apps.json", "direction": 1 } ] }
```

* `state` is a DaemonState file as for `--mock` (`lucide:` icons and relative asset paths work). Its
  `overlay.visible` is ignored; the `visible` events control it.
* `profile` sends the daemon's `profile` event followed by the new state, so the real switch animation runs.
* Frame `n` shows time `n / fps`: the clock advances, the frame's events are delivered, the window settles,
  and it is grabbed. An animation started by an event at frame `n` therefore shows its first step in `n + 1`
  (showing the panel takes one extra frame while the window maps).

Output: `0000.png …` (RGBA, constant window size, transparent while nothing is shown) and `manifest.json`
`{fps, frames, width, height, anchor}`. `anchor` is the pixel position in the window that must sit exactly on
the monitor's corner (bottom-right for the default bottom/end layout), so the panel's fillets meet the screen
edges. Animated GIF/WebP covers are not clock-driven and must not be used in captures.
