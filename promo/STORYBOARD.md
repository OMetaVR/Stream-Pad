# StreamPad promo — storyboard & production plan

Goal: a ~15 s seamless-looping README GIF that sells StreamPad: numpad → macropad, the on-screen pad, the
editor, the flow editor. Professional, smooth, engaging. **Every UI element is rendered by the real project
code** — the overlay by the real `streampad-overlay` binary, the editor by the real React components — so
sizes, fonts, colours and spacing are identical to the shipped app.

## Format

| | |
|---|---|
| Canvas | 1280 × 720 (16:9), captured at deviceScaleFactor 1 |
| Frame rate | 30 fps, 15.0 s → **450 frames** (frame `n` is time `n / 30` s) |
| Loop | last frame must flow into frame 0 (both are the empty dark stage) |
| Outputs | `promo/out/streampad.gif` (README, ≤ ~12 MB, 960 px wide), `streampad.mp4`, `streampad.webm` |
| Determinism | everything is a pure function of the frame number — no wall-clock animation in captured pixels |

## Look

- Stage: near-black graphite `#0a0b0e` with two very soft, large radial glows (violet `#8b7cf6` @ ~14 %,
  blue `#4aa8ff` @ ~8 %) drifting slowly. No noise/grain (GIF banding + size). Gentle vignette.
- Type: Inter Variable (the editor's bundled font). Captions 44–56 px semibold, tracking −0.02 em, white
  `#e9e9ef`; the accent word in `#b3a6ff`. Small labels 15 px medium `#8b8d98`.
- Motion: ease-out cubic/quint for entrances, ease-in for exits, 250–600 ms; springy overshoot only on the
  keycap "wave" (≤ 4 %). Nothing ever pops in without easing. Camera moves are slow (≥ 0.8 s).
- Accent colour per profile follows the app (Audio violet `#8b7cf6`, Apps blue `#5b9cf6`).

## Scenes (times in seconds; frames = t × 30)

### S1 — "Your numpad → a macropad" (0.00 – 2.40) · 3D
- 0.00–0.30 fade up from `#0a0b0e`.
- The editor's real numpad keybed (the `Keycap` visuals from `apps/editor/src/features/numpad/NumpadCanvas.tsx`,
  empty profile = legends only) floats in 3D: CSS `perspective: 1400px`, `rotateX(50°) rotateZ(-16°)`
  settling to `rotateX(38°) rotateZ(-10°)` across the scene, keys extruded with the keycap skirt for depth,
  soft contact shadow under the keybed. A separate **Scroll Lock** keycap with a small LED sits above-left.
- 0.45 caption (left third): **"Your numpad."**
- 1.05 Scroll Lock depresses (2 px + skirt shrink, 90 ms), LED lights accent violet with a bloom.
- 1.10–1.75 a wave flips every key from plain legend to the **Audio** profile faces (icons, labels, the
  synthwave cover on 9; globals show Prev/Next/Show-Hide), staggered by distance from Scroll Lock (≈ 45 ms
  per key), each a quick 3D flip/scale with ≤ 4 % overshoot.
- 1.30 caption becomes **"Your numpad. *Now a macropad.*"** (second line accent).
- 2.00–2.40 camera flattens and pulls back; crossfade into S2.

### S2 — The on-screen pad (2.40 – 6.60) · real overlay frames
- A desktop: 1280×720 "monitor" with a dark abstract wallpaper (navy→violet blobs, CSS), one large,
  out-of-focus app window (e.g. a music player / browser silhouette, blurred 10 px, 70 % opacity). No panel.
- **Key-cast HUD** bottom-left: each physical key press appears as a real-keycap-style chip ("Enter", "5",
  "+", "1") that pops in (scale 0.8→1, 160 ms) and fades after ~700 ms.
- Caption pill top-left (glass: `rgba(18,19,24,0.72)` + 1 px hairline + blur): "Press **Enter** — it slides in."
  → at 4.4 "**+ / −** switch profiles."
- Overlay events (rendered by the real overlay, composited so its window's bottom-right touches the
  monitor's bottom-right corner — the concave fillets must meet the screen edges):
  - 2.70 Enter → slide in (Audio profile)
  - 3.70 "5" → key 5 press glow + toast "Play / Pause"
  - 4.50 "+" → profile switch Audio → Apps (real slide/crossfade)
  - 5.30 "1" → key 1 press (Discord)
  - 6.10 Enter → slide out
- (6.30–6.60) desktop dims; editor window starts rising.

### S3 — The editor (6.60 – 11.60) · real editor UI
- The real editor (`apps/editor`, mock backend) at 1280×800 inside a window frame (rounded 12 px,
  hairline, big soft shadow, title "StreamPad"), scaled to ~1120 px wide, rises 40 px + fades in (0.5 s).
- Animated cursor (macOS-style arrow, white with dark outline, subtle shadow; click = 0.92 scale + ripple).
- 7.30 cursor → key **7** (empty) on the Audio pad; click → selection ring.
- 7.80 press Enter → the real **Action picker** opens; types "spot" (one char / 70 ms) → real fuzzy results.
- 8.50 Enter → "Open an app" step is added; the step form's App picker opens, "Spotify" highlighted, click.
  The key and inspector update live: icon + label "Spotify".
- 9.40 a file chip "synthwave.png" is dragged from off-screen onto key **8**; drop → cover set → the real
  **Crop dialog** opens; the frame's corner is dragged in (zoom ~1.6×) and moved; "Save".
- 10.9 key 8 shows the cropped cover. Caption: "Design every key." (top-left pill, as S2).

### S4 — The flow editor (11.60 – 13.90)
- Quick push-in transition to the real **Flow editor** for Audio · Numpad 4 (browser-aware back/prev).
- 12.2 cursor clicks **Test**; trace highlights step along trigger → If/else → "Press a shortcut"
  (≈ 280 ms per node, driven by the timeline via the daemon store's trace events).
- 13.0 tab flips to **Code** briefly, showing the generated JavaScript.
- Caption: "Branch, loop — or write JavaScript."

### S5 — Outro (13.90 – 15.00)
- Editor scales to 0.96 and fades; centred: the app icon (`packaging/icons/streampad.svg`, 96 px) +
  "StreamPad" wordmark + small "v2 · for Linux" in muted text, each easing in 80 ms apart.
- 14.55–15.00 fade to the empty stage so frame 449 → frame 0 loops seamlessly.

## Production pipeline

1. `promo/overlay/` — overlay frame sequences rendered by `streampad-overlay --capture <script.json>`
   (deterministic fixed-step animation clock), RGBA PNGs + `manifest.json` (window size, where the screen
   corner is inside the window, frame count, fps).
2. `apps/editor/promo.html` + `apps/editor/src/promo/**` — the "director": a page built from the editor's own
   components, with `window.promo.seek(frame)` that sets every scene element for that frame. All CSS
   transitions/animations are disabled in promo mode; motion comes only from the timeline.
3. `promo/capture.mjs` — Playwright (Chromium, 1280×720) seeks every frame and saves `promo/build/frames/%04d.png`.
4. `promo/encode.sh` — frames → GIF (gifski, tuned) + MP4 (x264) + WebM (VP9).

`promo/build/` and `promo/out/` are build outputs (the final GIF is copied to `docs/images/promo.gif`).
