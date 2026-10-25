#!/usr/bin/env python3
"""Generate the browser-mock assets in public/mock/ (committed; re-run only to change them).

    python3 -I scripts/gen-mock-assets.py <lucide icon dir>

* covers/: sample key covers — a synthwave PNG, an animated "live" GIF and an aurora WebP.
* apps/:   app icon tiles (gradient squircle + white lucide glyph) for the mock app list. Real app icons
           are trademarks, so the mock uses these stand-ins.
Requires Pillow.
"""

import math
import re
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter

OUT = Path(__file__).resolve().parent.parent / "public" / "mock"

# desktop id → (glyph, gradient top, gradient bottom)
APPS = {
    "firefox": ("flame", "#ff9f43", "#e8453c"),
    "discord": ("messages-square", "#7b86ff", "#4f5bd5"),
    "steam": ("gamepad-2", "#3d6fa8", "#1b2838"),
    "spotify": ("audio-lines", "#2fe07a", "#149c4a"),
    "org.kde.plasma-systemmonitor": ("activity", "#4fc3f7", "#1e88e5"),
    "org.kde.dolphin": ("folder-open", "#62b8ff", "#2c7be5"),
    "org.kde.konsole": ("terminal", "#4a4f5c", "#22252d"),
    "org.pulseaudio.pavucontrol": ("sliders-horizontal", "#9ccc65", "#558b2f"),
    "com.obsproject.Studio": ("radio-tower", "#5a5f6e", "#2a2d35"),
    "org.kde.kate": ("file-code-2", "#8e9cff", "#5865d6"),
    "org.kde.spectacle": ("scan", "#5ad1c9", "#1e9e95"),
    "systemsettings": ("settings", "#7f8796", "#4a5160"),
    "code": ("code-xml", "#3ea7f0", "#1565c0"),
    "chromium": ("globe", "#6fa8ff", "#3367d6"),
    "org.mozilla.Thunderbird": ("mail", "#5c9dff", "#2a5bd7"),
    "vlc": ("traffic-cone", "#ffa64d", "#f0730a"),
    "org.gimp.GIMP": ("palette", "#a1887f", "#6d4c41"),
    "org.inkscape.Inkscape": ("pen-tool", "#5f6b7a", "#2c3440"),
    "blender": ("orbit", "#ffa94d", "#e8711a"),
    "libreoffice-writer": ("file-text", "#55a7f5", "#1f6fd1"),
    "org.kde.kdenlive": ("clapperboard", "#6c7cff", "#3b47c4"),
    "org.kde.krita": ("brush", "#f06eaa", "#c2185b"),
    "signal-desktop": ("message-circle", "#5e8bff", "#3a62e0"),
    "org.telegram.desktop": ("send", "#4fc3f7", "#0288d1"),
    "org.kde.okular": ("book-open", "#ef6c6c", "#c62828"),
    "org.kde.elisa": ("music", "#ff8a65", "#e64a19"),
    "audacity": ("audio-waveform", "#7986cb", "#3949ab"),
    "bitwarden": ("shield", "#5c8df6", "#175ddc"),
}


def glyph_markup(lucide_dir: Path, name: str) -> str:
    svg = (lucide_dir / f"{name}.svg").read_text()
    inner = re.search(r"<svg[^>]*>(.*)</svg>", svg, re.S)
    if not inner:
        raise SystemExit(f"cannot parse lucide icon {name}")
    return inner.group(1).strip()


def app_tile(lucide_dir: Path, glyph: str, top: str, bottom: str) -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64" viewBox="0 0 64 64">
  <defs><linearGradient id="g" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="{top}"/><stop offset="1" stop-color="{bottom}"/></linearGradient></defs>
  <rect x="4" y="4" width="56" height="56" rx="15" fill="url(#g)"/>
  <rect x="4.5" y="4.5" width="55" height="55" rx="14.5" fill="none" stroke="#fff" stroke-opacity=".18"/>
  <g transform="translate(16 16) scale(1.3333)" fill="none" stroke="#fff" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{glyph_markup(lucide_dir, glyph)}</g>
</svg>
"""


def lerp(a, b, t):
    return tuple(round(x + (y - x) * t) for x, y in zip(a, b))


def hex_rgb(h: str):
    return tuple(int(h[i : i + 2], 16) for i in (1, 3, 5))


def synthwave(size=256) -> Image.Image:
    img = Image.new("RGB", (size, size))
    d = ImageDraw.Draw(img)
    stops = [hex_rgb("#1a0b3d"), hex_rgb("#6b1f7a"), hex_rgb("#e0457b"), hex_rgb("#ffb347")]
    horizon = int(size * 0.62)
    for y in range(horizon):
        t = y / horizon * (len(stops) - 1)
        i = min(int(t), len(stops) - 2)
        d.line([(0, y), (size, y)], fill=lerp(stops[i], stops[i + 1], t - i))
    # striped sun
    cx, cy, r = size // 2, int(size * 0.5), int(size * 0.24)
    sun = Image.new("L", (size, size), 0)
    ImageDraw.Draw(sun).ellipse([cx - r, cy - r, cx + r, cy + r], fill=255)
    sd = ImageDraw.Draw(sun)
    for k in range(6):
        y = cy + int(r * 0.15) + k * int(r * 0.16)
        sd.rectangle([0, y, size, y + 2 + k], fill=0)
    sun_color = Image.new("RGB", (size, size), hex_rgb("#ffd166"))
    img.paste(sun_color, (0, 0), sun)
    # floor grid
    d.rectangle([0, horizon, size, size], fill=hex_rgb("#12062b"))
    grid = hex_rgb("#ff4fd8")
    for k in range(1, 9):
        y = horizon + int((size - horizon) * (k / 8) ** 2)
        d.line([(0, y), (size, y)], fill=grid, width=1)
    for k in range(-8, 9):
        d.line([(cx + k * 6, horizon), (cx + k * 48, size)], fill=grid, width=1)
    glow = img.filter(ImageFilter.GaussianBlur(1.2))
    return Image.blend(img, glow, 0.35)


def live_gif(size=128, frames=18):
    out = []
    for f in range(frames):
        t = f / frames
        img = Image.new("RGB", (size, size), hex_rgb("#16171d"))
        d = ImageDraw.Draw(img)
        cx = cy = size // 2
        for ring in range(3):
            p = (t + ring / 3) % 1
            r = 10 + p * 46
            a = 1 - p
            col = lerp(hex_rgb("#16171d"), hex_rgb("#ff4d5e"), a * 0.9)
            d.ellipse([cx - r, cy - r, cx + r, cy + r], outline=col, width=3)
        pulse = 11 + 2 * math.sin(t * 2 * math.pi)
        d.ellipse([cx - pulse, cy - pulse, cx + pulse, cy + pulse], fill=hex_rgb("#ff4d5e"))
        out.append(img.convert("P", palette=Image.ADAPTIVE, colors=64))
    return out


def aurora(size=256) -> Image.Image:
    img = Image.new("RGB", (size, size), hex_rgb("#061420"))
    layer = Image.new("RGB", (size, size), (0, 0, 0))
    d = ImageDraw.Draw(layer)
    for i, (col, x, y, r) in enumerate(
        [("#19d3a2", 0.25, 0.35, 0.42), ("#3a7bff", 0.75, 0.3, 0.38), ("#9b5cff", 0.55, 0.75, 0.4), ("#0fb3c9", 0.2, 0.8, 0.3)]
    ):
        cx, cy, rr = x * size, y * size, r * size
        d.ellipse([cx - rr, cy - rr, cx + rr, cy + rr], fill=hex_rgb(col))
    layer = layer.filter(ImageFilter.GaussianBlur(size * 0.12))
    return Image.blend(img, layer, 0.85)


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    lucide_dir = Path(sys.argv[1])
    (OUT / "covers").mkdir(parents=True, exist_ok=True)
    (OUT / "apps").mkdir(parents=True, exist_ok=True)
    synthwave().save(OUT / "covers" / "synthwave.png", optimize=True)
    frames = live_gif()
    frames[0].save(OUT / "covers" / "live.gif", save_all=True, append_images=frames[1:], duration=70, loop=0, optimize=True)
    aurora().save(OUT / "covers" / "aurora.webp", quality=82)
    for app_id, (glyph, top, bottom) in APPS.items():
        (OUT / "apps" / f"{app_id}.svg").write_text(app_tile(lucide_dir, glyph, top, bottom))
    print(f"wrote {len(APPS)} app tiles and 3 covers to {OUT}")


if __name__ == "__main__":
    main()
