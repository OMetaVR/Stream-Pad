#!/usr/bin/env python3
"""Generates the overlay test assets (animated GIF, photo-like PNG, colour app icon) and the
DaemonState fixtures in test/fixtures/. Re-run after changing fixtures.py."""

from __future__ import annotations

import colorsys
import copy
import json

import numpy as np
from PIL import Image, ImageDraw, ImageFilter

import fixtures as fx


def hsv_to_rgb(h: np.ndarray, s: float, v: np.ndarray) -> np.ndarray:
    """Vectorised HSV → RGB (all inputs in 0..1)."""
    i = np.floor(h * 6).astype(int) % 6
    f = h * 6 - np.floor(h * 6)
    p, q, t = v * (1 - s), v * (1 - s * f), v * (1 - s * (1 - f))
    choices = [(v, t, p), (q, v, p), (p, v, t), (p, q, v), (t, p, v), (v, p, q)]
    rgb = np.zeros(h.shape + (3,))
    for k, (r, g, b) in enumerate(choices):
        mask = i == k
        rgb[mask] = np.stack([r[mask], g[mask], b[mask]], axis=-1)
    return rgb


def animated_gradient(path, size: int = 120, frames: int = 36) -> None:
    y, x = np.mgrid[0:size, 0:size] / (size - 1)
    images = []
    for n in range(frames):
        t = n / frames
        hue = (0.55 * x + 0.35 * y + t) % 1.0
        # A soft highlight orbiting the centre.
        cx, cy = 0.5 + 0.28 * np.cos(2 * np.pi * t), 0.5 + 0.28 * np.sin(2 * np.pi * t)
        glow = np.exp(-((x - cx) ** 2 + (y - cy) ** 2) / 0.05)
        value = np.clip(0.78 + 0.22 * glow, 0, 1)
        rgb = hsv_to_rgb(hue, 0.72, value)
        rgb = rgb * (1 - 0.35 * glow[..., None]) + 0.35 * glow[..., None]
        frame = Image.fromarray((np.clip(rgb, 0, 1) * 255).astype(np.uint8), "RGB")
        images.append(frame.quantize(colors=128, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.FLOYDSTEINBERG))
    images[0].save(path, save_all=True, append_images=images[1:], duration=50, loop=0, optimize=True, disposal=1)


def lerp(a, b, t):
    return a + (b - a) * t


def photo(path, width: int = 320, height: int = 320) -> None:
    """A dusk landscape: graded sky, a glowing sun, layered ridges and film grain."""
    rng = np.random.default_rng(7)
    y, x = np.mgrid[0:height, 0:width]
    u, v = x / (width - 1), y / (height - 1)

    stops = [(0.0, (24, 26, 74)), (0.42, (122, 54, 120)), (0.62, (232, 104, 96)), (0.74, (255, 184, 104))]
    sky = np.zeros((height, width, 3))
    for (t0, c0), (t1, c1) in zip(stops, stops[1:]):
        band = (v >= t0) & (v <= t1)
        k = ((v - t0) / (t1 - t0))[band][:, None]
        sky[band] = lerp(np.array(c0, float), np.array(c1, float), k)
    sky[v > stops[-1][0]] = stops[-1][1]

    sun_x, sun_y = 0.64, 0.6
    d = np.sqrt((u - sun_x) ** 2 + ((v - sun_y) * 1.0) ** 2)
    sky += (np.exp(-d * 9) * 140)[..., None] * np.array([1.0, 0.75, 0.45])
    sky[d < 0.075] = (255, 236, 196)

    ridges = [(0.66, 0.06, (92, 46, 92), 3.1), (0.74, 0.05, (58, 30, 70), 5.3), (0.84, 0.04, (30, 18, 44), 8.7)]
    for base, amp, color, freq in ridges:
        line = base - amp * (np.sin(u * freq + base * 9) * 0.6 + np.sin(u * freq * 2.3 + 1.7) * 0.4)
        sky[v > line] = color
        haze = np.clip((v - line) * 6, 0, 1)[..., None]
        sky = np.where((v > line)[..., None], sky * (1 - 0.15 * haze), sky)

    sky += rng.normal(0, 5, sky.shape)
    image = Image.fromarray(np.clip(sky, 0, 255).astype(np.uint8), "RGB").filter(ImageFilter.GaussianBlur(0.6))
    image.save(path, optimize=True)


def app_icon(path, size: int = 128) -> None:
    """A colourful rounded-square app icon (drawn 4× and downsampled for smooth edges)."""
    big = size * 4
    gradient = np.zeros((big, big, 4), np.uint8)
    t = np.linspace(0, 1, big)[:, None]
    top, bottom = np.array(colorsys.hsv_to_rgb(0.47, 0.75, 0.95)), np.array(colorsys.hsv_to_rgb(0.62, 0.8, 0.9))
    gradient[..., :3] = (lerp(top, bottom, t) * 255).astype(np.uint8)[:, None, :]
    gradient[..., 3] = 255
    icon = Image.fromarray(gradient, "RGBA")
    mask = Image.new("L", (big, big), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, big - 1, big - 1), radius=big * 0.23, fill=255)
    icon.putalpha(mask)
    draw = ImageDraw.Draw(icon)
    c = big / 2
    draw.ellipse((c - big * 0.27, c - big * 0.27, c + big * 0.27, c + big * 0.27), outline=(255, 255, 255, 255),
                 width=int(big * 0.07))
    draw.polygon([(c - big * 0.08, c - big * 0.13), (c - big * 0.08, c + big * 0.13), (c + big * 0.14, c)],
                 fill=(255, 255, 255, 255))
    icon.resize((size, size), Image.Resampling.LANCZOS).save(path, optimize=True)


def write(name: str, state: dict) -> None:
    path = fx.FIXTURES_DIR / f"{name}.json"
    path.write_text(json.dumps(state, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print("wrote", path.relative_to(fx.HERE))


def main() -> None:
    fx.ASSETS_DIR.mkdir(exist_ok=True)
    fx.FIXTURES_DIR.mkdir(exist_ok=True)
    animated_gradient(fx.ASSETS_DIR / "gradient.gif")
    photo(fx.ASSETS_DIR / "photo.png")
    app_icon(fx.ASSETS_DIR / "app-icon.png")

    profiles = fx.with_stream(fx.PROFILES)
    stream = len(profiles) - 1

    write("dark", fx.daemon_state(profiles, 0))
    write("light", fx.daemon_state(profiles, 0, settings={"theme": "light"}))
    write("apps", fx.daemon_state(profiles, 1))
    write("covers", fx.daemon_state(profiles, stream))
    write("covers-light", fx.daemon_state(profiles, stream, settings={"theme": "light"}))
    for edge in ("bottom", "top", "left", "right"):
        for align in ("start", "center", "end"):
            write(f"edge-{edge}-{align}", fx.daemon_state(profiles, 2, settings={"edge": edge, "align": align}))
    write("offset", fx.daemon_state(profiles, 3, settings={"offset": 120}))
    write("legends-off", fx.daemon_state(profiles, 0, settings={"showLegends": False}))
    write("legends-number", fx.daemon_state(profiles, 4, settings={"legendStyle": "number"}))
    write("minimal", fx.daemon_state(profiles, 1, settings={"showHeader": False, "showLabels": False}))
    write("scale-125", fx.daemon_state(profiles, stream, settings={"scale": 1.25, "opacity": 0.82}))

    many = copy.deepcopy(profiles)
    for n in range(6):
        clone = copy.deepcopy(fx.PROFILES[n % len(fx.PROFILES)])
        clone["id"], clone["name"] = f"p_extra{n}", f"{clone['name']} {n + 2}"
        many.append(clone)
    write("many-profiles", fx.daemon_state(many, 9))
    write("hidden", fx.daemon_state(profiles, 0, visible=False, pinned=False))


if __name__ == "__main__":
    main()
