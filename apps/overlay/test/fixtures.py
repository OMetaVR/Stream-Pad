"""Shared fixture data for the overlay tests: render profiles and DaemonState builders (docs/SPEC.md §4).

Icons are written as "lucide:<name>" and assets relative to the fixtures directory; `streampad-overlay
--mock` resolves both. The fake daemon resolves them to absolute paths, like the real daemon does.
"""

from __future__ import annotations

import copy
import os
from pathlib import Path
from typing import Callable

HERE = Path(__file__).resolve().parent
FIXTURES_DIR = HERE / "fixtures"
ASSETS_DIR = HERE / "assets"
REPO_ROOT = HERE.parents[2]

# KeyId, legend main, legend sub — the faithful numpad of docs/SPEC.md §3.1.
KEYS = [
    ("NumLock", "Num", "Lock"),
    ("KPSlash", "/", None),
    ("KPAsterisk", "*", None),
    ("KPMinus", "−", None),
    ("KP7", "7", "Home"),
    ("KP8", "8", "↑"),
    ("KP9", "9", "PgUp"),
    ("KPPlus", "+", None),
    ("KP4", "4", "←"),
    ("KP5", "5", None),
    ("KP6", "6", "→"),
    ("KP1", "1", "End"),
    ("KP2", "2", "↓"),
    ("KP3", "3", "PgDn"),
    ("KPEnter", "Enter", None),
    ("KP0", "0", "Ins"),
    ("KPDot", ".", "Del"),
]

# Default globals (§2.2) as the daemon renders them (§4 "Global keys in RenderKey").
GLOBALS = {
    "KPEnter": ("Show / Hide", "eye", "toggleOverlay"),
    "KPPlus": ("Next", "chevron-down", "nextProfile"),
    "KPMinus": ("Prev", "chevron-up", "prevProfile"),
}


def bind(label: str, icon: str | None = None, cover: str | None = None, color: str | None = None) -> dict:
    return {"label": label, "icon": f"lucide:{icon}" if icon and ":" not in icon and "/" not in icon else icon,
            "cover": cover, "color": color}


PROFILES = [
    {
        "id": "p_audio", "name": "Audio", "color": None, "icon": "lucide:audio-lines",
        "keys": {
            "KP5": bind("Play / Pause", "play"),
            "KP4": bind("Previous", "skip-back"),
            "KP6": bind("Next track", "skip-forward"),
            "KP8": bind("Volume up", "volume-2"),
            "KP2": bind("Volume down", "volume-1"),
            "KP0": bind("Mute", "volume-x"),
            "KP3": bind("Game mode", "gamepad-2"),
        },
    },
    {
        "id": "p_apps", "name": "Apps", "color": "#4fb3ff", "icon": "lucide:layout-grid",
        "keys": {
            "KP1": bind("Discord", "message-circle"),
            "KP2": bind("Firefox", "globe"),
            "KP3": bind("Steam", "gamepad"),
            "KP4": bind("Spotify", "music"),
            "KP5": bind("Monitor", "activity"),
            "KP6": bind("Files", "folder"),
            "KP7": bind("Terminal", "terminal"),
            "KP8": bind("Mixer", "sliders-horizontal"),
        },
    },
    {
        "id": "p_web", "name": "Web", "color": None, "icon": "lucide:globe",
        "keys": {
            "KP1": bind("YouTube", "circle-play"),
            "KP2": bind("Google", "search"),
            "KP4": bind("GitHub", "git-branch"),
            "KP5": bind("Discord", "message-circle"),
            "KP7": bind("Twitch", "cast"),
            "KP8": bind("Kick", "zap"),
            "KP0": bind("ChatGPT", "sparkles"),
        },
    },
    {
        "id": "p_windows", "name": "Windows", "color": "#3ecf8e", "icon": "lucide:app-window",
        "keys": {
            "KP4": bind("Monitor left", "arrow-left-to-line"),
            "KP6": bind("Monitor right", "arrow-right-to-line"),
            "KP8": bind("Monitor up", "arrow-up-to-line"),
            "KP2": bind("Monitor down", "arrow-down-to-line"),
            "KP5": bind("Maximize", "maximize-2"),
            "KP7": bind("Minimize", "minimize-2"),
            "KP9": bind("Keep above", "pin"),
            "KP1": bind("Center", "locate"),
            "KP3": bind("Fullscreen", "expand"),
        },
    },
    {
        "id": "p_desktops", "name": "Desktops", "color": None, "icon": "lucide:layers",
        "keys": {
            "KP4": bind("Desktop left", "chevrons-left"),
            "KP6": bind("Desktop right", "chevrons-right"),
            "KP1": bind("Send left", "arrow-big-left"),
            "KP3": bind("Send right", "arrow-big-right"),
            "KP8": bind("New desktop", "square-plus"),
            "KP2": bind("Remove", "square-minus"),
            "KP5": bind("Overview", "layout-dashboard"),
        },
    },
]

# A streaming profile exercising covers (photo PNG + animated GIF), key tints, a colour app icon,
# a label-only key and a long label.
STREAM_PROFILE = {
    "id": "p_stream", "name": "Stream", "color": "#ff7a59", "icon": "lucide:radio-tower",
    "keys": {
        "KP7": bind("Scene: Chat", cover="../assets/photo.png"),
        "KP8": bind("Hype", cover="../assets/gradient.gif"),
        "KP9": bind("Clip it", "scissors"),
        "KP4": bind("Mic", "mic", color="#ff7a59"),
        "KP5": bind("Camera", "camera", color="#4fb3ff"),
        "KP6": bind("Studio", "../assets/app-icon.png"),
        "KP1": bind("Be right back"),
        "KP2": bind("Start recording now", "circle-play"),
        "KP3": bind("Mute desktop audio", "volume-x", color="#f2555a"),
        "NumLock": bind("Live", "radio"),
        "KP0": bind("Highlight reel", cover="../assets/gradient.gif"),
    },
}


def default_settings() -> dict:
    """OverlaySettings defaults (docs/SPEC.md §3.2)."""
    return {
        "edge": "bottom", "align": "end", "offset": 0, "monitor": None, "scale": 1.0, "theme": "dark",
        "accent": "#8b7cf6", "opacity": 0.94, "blur": True, "showHeader": True, "showLegends": True,
        "legendStyle": "full", "showLabels": True, "animationMs": 320, "peekOnModeChange": True,
        "peekOnProfileChange": True, "peekMs": 1600,
    }


Resolver = Callable[[str | None], str | None]


def keep(ref: str | None) -> str | None:
    return ref


def absolute_resolver(share_dir: Path | None) -> Resolver:
    """Resolves fixture refs the way the daemon does: lucide icons from the share dir, assets to absolute paths."""
    def resolve(ref: str | None) -> str | None:
        if not ref:
            return None
        if ref.startswith("lucide:"):
            if share_dir is None:
                return None
            path = share_dir / "icons" / "lucide" / f"{ref[len('lucide:'):]}.svg"
            return str(path) if path.is_file() else None
        path = Path(ref)
        return str(path if path.is_absolute() else (FIXTURES_DIR / path).resolve())
    return resolve


def find_share_dir() -> Path | None:
    for candidate in (os.environ.get("STREAMPAD_SHARE_DIR"), REPO_ROOT / "build" / "share" / "streampad"):
        if candidate and (Path(candidate) / "icons").is_dir():
            return Path(candidate)
    return None


def render_profile(profile: dict, resolve: Resolver = keep) -> dict:
    keys = {}
    for key_id, main, sub in KEYS:
        legend = {"main": main, "sub": sub}
        if key_id in GLOBALS:
            label, icon, action = GLOBALS[key_id]
            keys[key_id] = {"label": label, "icon": resolve(f"lucide:{icon}"), "iconTint": True, "cover": None,
                            "color": None, "kind": "global", "global": action, "legend": legend}
            continue
        binding = profile["keys"].get(key_id)
        if binding is None:
            keys[key_id] = {"label": "", "icon": None, "iconTint": False, "cover": None, "color": None,
                            "kind": "empty", "global": None, "legend": legend}
            continue
        icon = binding.get("icon")
        keys[key_id] = {
            "label": binding["label"],
            "icon": resolve(icon),
            "iconTint": bool(icon and icon.startswith("lucide:")),
            "cover": resolve(binding.get("cover")),
            "color": binding.get("color"),
            "kind": "macro",
            "global": None,
            "legend": legend,
        }
    return {"id": profile["id"], "name": profile["name"], "color": profile["color"],
            "icon": resolve(profile["icon"]), "keys": keys}


def daemon_state(profiles: list[dict], index: int = 0, *, settings: dict | None = None, visible: bool = True,
                 pinned: bool = True, output: str | None = None, resolve: Resolver = keep) -> dict:
    overlay_settings = default_settings()
    overlay_settings.update(settings or {})
    profile = profiles[index]
    return {
        "version": "0.1.0",
        "macroMode": True,
        "profileIndex": index,
        "profiles": [{"id": p["id"], "name": p["name"], "color": p["color"], "icon": resolve(p["icon"])}
                     for p in profiles],
        "profile": render_profile(profile, resolve),
        "overlay": {
            "visible": visible,
            "pinned": pinned,
            "preview": False,
            "output": output,
            "settings": overlay_settings,
            "accent": profile["color"] or overlay_settings["accent"],
        },
        "learn": False,
        "devices": [],
        "integrations": {"kwin": True, "layerShell": True},
        "configError": None,
    }


def with_stream(profiles: list[dict]) -> list[dict]:
    return copy.deepcopy(profiles) + [copy.deepcopy(STREAM_PROFILE)]
