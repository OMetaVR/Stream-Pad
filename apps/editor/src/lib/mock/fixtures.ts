// Static discovery data for the browser mock, modelled on the dev machine (two 2560×1440 monitors,
// a Razer keyboard, a KDE Plasma session).

import type { AppEntry, KdeShortcut } from "../backend";
import type { DeviceInfo, OutputInfo, WindowInfo } from "../types";

type AppSeed = [id: string, name: string, description: string, categories: string, wmClass?: string];

const APP_SEEDS: AppSeed[] = [
  ["audacity", "Audacity", "Audio Editor", "AudioVideo;Audio;AudioVideoEditing"],
  ["bitwarden", "Bitwarden", "Password Manager", "Utility"],
  ["blender", "Blender", "3D Modeler", "Graphics;3DGraphics"],
  ["chromium", "Chromium", "Web Browser", "Network;WebBrowser", "chromium"],
  ["code", "Visual Studio Code", "Code Editor", "Development;IDE", "code"],
  ["discord", "Discord", "Internet Messenger", "Network;InstantMessaging", "discord"],
  ["org.kde.dolphin", "Dolphin", "File Manager", "Qt;KDE;System;FileTools;FileManager", "dolphin"],
  ["org.kde.elisa", "Elisa", "Music Player", "Qt;KDE;AudioVideo;Player"],
  ["firefox", "Firefox", "Web Browser", "Network;WebBrowser", "firefox"],
  ["org.gimp.GIMP", "GNU Image Manipulation Program", "Image Editor", "Graphics;2DGraphics;RasterGraphics", "gimp"],
  ["org.inkscape.Inkscape", "Inkscape", "Vector Graphics Editor", "Graphics;VectorGraphics"],
  ["org.kde.kate", "Kate", "Advanced Text Editor", "Qt;KDE;Utility;TextEditor", "kate"],
  ["org.kde.kdenlive", "Kdenlive", "Video Editor", "Qt;KDE;AudioVideo;AudioVideoEditing"],
  ["org.kde.konsole", "Konsole", "Terminal", "Qt;KDE;System;TerminalEmulator", "konsole"],
  ["org.kde.krita", "Krita", "Digital Painting", "Qt;KDE;Graphics;RasterGraphics", "krita"],
  ["libreoffice-writer", "LibreOffice Writer", "Word Processor", "Office;WordProcessor", "libreoffice-writer"],
  ["com.obsproject.Studio", "OBS Studio", "Streaming/Recording Software", "AudioVideo;Recorder", "obs"],
  ["org.kde.okular", "Okular", "Document Viewer", "Qt;KDE;Graphics;Office;Viewer", "okular"],
  ["org.pulseaudio.pavucontrol", "Volume Control", "Adjust the volume level", "AudioVideo;Audio;Mixer", "pavucontrol"],
  ["signal-desktop", "Signal", "Private messenger", "Network;InstantMessaging;Chat", "signal"],
  ["org.kde.spectacle", "Spectacle", "Screenshot Capture Utility", "Qt;KDE;Utility", "spectacle"],
  ["spotify", "Spotify", "Music Player", "Audio;Music;Player;AudioVideo", "spotify"],
  ["steam", "Steam", "Application for managing and playing games on Steam", "Network;FileTransfer;Game", "steam"],
  ["org.kde.plasma-systemmonitor", "System Monitor", "System Monitor", "Qt;KDE;System;Monitor", "plasma-systemmonitor"],
  ["systemsettings", "System Settings", "Configure your system", "Qt;KDE;Settings", "systemsettings"],
  ["org.telegram.desktop", "Telegram", "New era of messaging", "Chat;Network;InstantMessaging", "telegramdesktop"],
  ["org.mozilla.Thunderbird", "Thunderbird", "Mail Client", "Network;Email", "thunderbird"],
  ["vlc", "VLC media player", "Media player", "AudioVideo;Player;Recorder", "vlc"],
];

export const MOCK_APPS: AppEntry[] = APP_SEEDS.map(([id, name, description, categories, wmClass]) => ({
  id,
  name,
  description,
  iconPath: `/mock/apps/${id}.svg`,
  iconUrl: `/mock/apps/${id}.svg`,
  categories: categories.split(";"),
  wmClass: wmClass ?? null,
})).sort((a, b) => a.name.localeCompare(b.name));

export const MOCK_OUTPUTS: OutputInfo[] = [
  { name: "DP-1", x: 0, y: 0, width: 2560, height: 1440, scale: 1, primary: false, model: "LG Electronics 27GL850" },
  { name: "DP-2", x: 2560, y: 0, width: 2560, height: 1440, scale: 1, primary: true, model: "Dell S2721DGF" },
];

export const MOCK_DEVICES: DeviceInfo[] = [
  {
    id: "1532:023f:Razer Razer Cynosa Lite",
    name: "Razer Razer Cynosa Lite",
    path: "/dev/input/event4",
    phys: "usb-0000:0c:00.3-2/input0",
    hasNumpad: true,
    eligible: true,
    grabbed: true,
    error: null,
  },
  {
    id: "1532:023f:Razer Razer Cynosa Lite Keyboard",
    name: "Razer Razer Cynosa Lite Keyboard",
    path: "/dev/input/event6",
    phys: "usb-0000:0c:00.3-2/input2",
    hasNumpad: true,
    eligible: true,
    grabbed: true,
    error: null,
  },
  {
    id: "046d:c539:Logitech USB Receiver Keyboard",
    name: "Logitech USB Receiver Keyboard",
    path: "/dev/input/event9",
    phys: "usb-0000:0c:00.4-1/input1",
    hasNumpad: false,
    eligible: false,
    grabbed: false,
    error: null,
  },
  {
    id: "0000:0000:Power Button",
    name: "Power Button",
    path: "/dev/input/event1",
    phys: "PNP0C0C/button/input0",
    hasNumpad: false,
    eligible: false,
    grabbed: false,
    error: null,
  },
];

export const MOCK_WINDOWS: WindowInfo[] = [
  {
    id: "{6b1f0e2a-0001}",
    title: "StreamPad — Mozilla Firefox",
    appId: "firefox",
    resourceClass: "firefox",
    desktopFile: "firefox",
    pid: 4121,
    output: "DP-2",
    active: true,
  },
  {
    id: "{6b1f0e2a-0002}",
    title: "~/Projects/streampad : zsh — Konsole",
    appId: "org.kde.konsole",
    resourceClass: "konsole",
    desktopFile: "org.kde.konsole",
    pid: 3990,
    output: "DP-1",
    active: false,
  },
  {
    id: "{6b1f0e2a-0003}",
    title: "Discord | #general",
    appId: "discord",
    resourceClass: "discord",
    desktopFile: "discord",
    pid: 5210,
    output: "DP-1",
    active: false,
  },
  {
    id: "{6b1f0e2a-0004}",
    title: "Spotify Premium",
    appId: "spotify",
    resourceClass: "spotify",
    desktopFile: "spotify",
    pid: 6033,
    output: "DP-1",
    active: false,
  },
];

type ShortcutSeed = [component: string, componentName: string, name: string, friendlyName: string, keys: string[]];

const SHORTCUT_SEEDS: ShortcutSeed[] = [
  ["kwin", "KWin", "Overview", "Toggle Overview", ["Meta+W"]],
  ["kwin", "KWin", "Grid View", "Toggle Grid View", ["Meta+G"]],
  ["kwin", "KWin", "ExposeAll", "Toggle Present Windows (All desktops)", ["Ctrl+F10", "Launch (C)"]],
  ["kwin", "KWin", "Expose", "Toggle Present Windows (Current desktop)", ["Ctrl+F9"]],
  ["kwin", "KWin", "Show Desktop", "Peek at Desktop", ["Meta+D"]],
  ["kwin", "KWin", "Window Close", "Close Window", ["Alt+F4"]],
  ["kwin", "KWin", "Window Maximize", "Maximize Window", ["Meta+PgUp"]],
  ["kwin", "KWin", "Window Minimize", "Minimize Window", ["Meta+PgDown"]],
  ["kwin", "KWin", "Window Quick Tile Left", "Quick Tile Window to the Left", ["Meta+Left"]],
  ["kwin", "KWin", "Window Quick Tile Right", "Quick Tile Window to the Right", ["Meta+Right"]],
  ["kwin", "KWin", "Window to Next Screen", "Move Window to Next Screen", ["Meta+Shift+Right"]],
  ["kwin", "KWin", "Switch to Desktop 1", "Switch to Desktop 1", ["Ctrl+F1"]],
  ["kwin", "KWin", "Switch to Desktop 2", "Switch to Desktop 2", ["Ctrl+F2"]],
  ["kwin", "KWin", "Switch One Desktop to the Right", "Switch One Desktop to the Right", ["Meta+Ctrl+Right"]],
  ["kwin", "KWin", "Switch One Desktop to the Left", "Switch One Desktop to the Left", ["Meta+Ctrl+Left"]],
  ["kwin", "KWin", "view_zoom_in", "Zoom In", ["Meta++", "Meta+="]],
  ["kwin", "KWin", "view_zoom_out", "Zoom Out", ["Meta+-"]],
  ["plasmashell", "Plasma Workspace", "activate application launcher", "Activate Application Launcher", ["Meta", "Alt+F1"]],
  ["plasmashell", "Plasma Workspace", "show-on-mouse-pos", "Show Clipboard Items at Mouse Position", ["Meta+V"]],
  ["plasmashell", "Plasma Workspace", "manage activities", "Show Activity Switcher", ["Meta+Q"]],
  ["ksmserver", "Session Management", "Lock Session", "Lock Session", ["Meta+L", "Screensaver"]],
  ["ksmserver", "Session Management", "Log Out", "Show Logout Screen", ["Ctrl+Alt+Del"]],
  ["kmix", "Audio Volume", "increase_volume", "Increase Volume", ["Volume Up"]],
  ["kmix", "Audio Volume", "decrease_volume", "Decrease Volume", ["Volume Down"]],
  ["kmix", "Audio Volume", "mute", "Mute", ["Volume Mute"]],
  ["kmix", "Audio Volume", "mic_mute", "Mute Microphone", ["Mic Mute", "Meta+Volume Mute"]],
  ["mediacontrol", "Media Controller", "playpausemedia", "Play/Pause media playback", ["Media Play"]],
  ["mediacontrol", "Media Controller", "nextmedia", "Media playback next", ["Media Next"]],
  ["org_kde_powerdevil", "Power Management", "Decrease Screen Brightness", "Decrease Screen Brightness", ["Monitor Brightness Down"]],
  ["org.kde.spectacle.desktop", "Spectacle", "RectangularRegionScreenShot", "Capture Rectangular Region", ["Meta+Shift+Print"]],
  ["org.kde.spectacle.desktop", "Spectacle", "_launch", "Launch Spectacle", ["Print"]],
  ["org.kde.konsole.desktop", "Konsole", "_launch", "Launch Konsole", ["Ctrl+Alt+T"]],
];

export const MOCK_KDE_SHORTCUTS: KdeShortcut[] = SHORTCUT_SEEDS.map(([component, componentName, name, friendlyName, keys]) => ({
  component,
  componentName,
  name,
  friendlyName,
  keys,
}));
