<div align="center">

# 🎮 Stream Pad

### *Transform your numpad into a powerful macro controller*

[![Downloads](https://img.shields.io/github/downloads/OMetaVR/Macro-NUM-pad/total?style=for-the-badge&logo=github&color=blue)](https://github.com/OMetaVR/Macro-NUM-pad/releases)
[![Contributors](https://img.shields.io/github/contributors/OMetaVR/Macro-NUM-pad?style=for-the-badge&logo=github&color=green)](https://github.com/OMetaVR/Macro-NUM-pad/graphs/contributors)
[![Stars](https://img.shields.io/github/stars/OMetaVR/Macro-NUM-pad?style=for-the-badge&logo=github&color=yellow)](https://github.com/OMetaVR/Macro-NUM-pad/stargazers)
[![Issues](https://img.shields.io/github/issues/OMetaVR/Macro-NUM-pad?style=for-the-badge&logo=github&color=red)](https://github.com/OMetaVR/Macro-NUM-pad/issues)
[![License](https://img.shields.io/github/license/OMetaVR/Macro-NUM-pad?style=for-the-badge&logo=github&color=purple)](https://github.com/OMetaVR/Macro-NUM-pad/blob/main/LICENSE)

[🐛 Report Bug](https://github.com/OMetaVR/Macro-NUM-pad/issues) • [✨ Request Feature](https://github.com/OMetaVR/Macro-NUM-pad/issues) • [📖 Documentation](https://github.com/OMetaVR/Macro-NUM-pad/wiki)

</div>

---

## 🚀 What is Stream Pad?

Stream Pad is a powerful AutoHotkey script that transforms your numpad into a versatile macro controller with **5 distinct profiles**. Whether you're streaming, gaming, or just want quick access to your favorite apps and websites, Stream Pad has you covered!

### ✨ Key Highlights

- 🎯 **5 Specialized Profiles**: Audio, Tasks, Websites, Hotswap, and Desktops
- 🔄 **Smart Profile Switching**: Seamless transitions with visual feedback
- 🎮 **Game Mode**: Special gaming optimizations
- 🖥️ **Multi-Monitor Support**: Effortless window management
- 🌐 **Browser Intelligence**: Context-aware browser controls
- 📱 **Visual Tooltips**: Real-time feedback for all actions

---

## 📋 Table of Contents

- [🚀 What is Stream Pad?](#-what-is-stream-pad)
- [⚡ Quick Start](#-quick-start)
- [🎛️ Profile Overview](#️-profile-overview)
- [🔧 Installation](#-installation)
- [📖 Usage Guide](#-usage-guide)
- [⚙️ Customization](#️-customization)
- [🎮 Game Mode](#-game-mode)
- [🤝 Contributing](#-contributing)
- [📄 License](#-license)

---

## ⚡ Quick Start

1. **Download** the latest release or clone this repository
2. **Install** [AutoHotkey](https://www.autohotkey.com/) if you haven't already
3. **Run** `macropad.ahk` by double-clicking it
4. **Press** `ScrollLock` to activate Stream Pad
5. **Use** `Numpad+` and `Numpad-` to switch between profiles
6. **Enjoy** your enhanced productivity! 🎉

---

## 🎛️ Profile Overview

<details>
<summary><strong>🎵 Audio Profile</strong> - Media & Volume Control</summary>

| Key | Action | Description |
|-----|--------|-------------|
| `5` | ⏯️ Play/Pause | Toggle media playback |
| `4` | ⏮️ Previous | Previous track (or browser back) |
| `6` | ⏭️ Next | Next track (or browser forward) |
| `8` | 🔊 Volume Up | Increase system volume |
| `2` | 🔉 Volume Down | Decrease system volume |
| `0` | 🔇 Mute | Toggle system mute |
| `1` | 🪟 Windows+B | System tray focus |
| `3` | 🎮 Game Mode | Toggle gaming optimizations |

</details>

<details>
<summary><strong>📱 Tasks Profile</strong> - Application Launcher</summary>

| Key | Action | Application |
|-----|--------|-------------|
| `1` | 💬 Discord | Launch or focus Discord |
| `2` | 🌐 Opera GX | Launch or focus Opera browser |
| `3` | 🎮 Steam | Launch or focus Steam |
| `4` | 🎵 Spotify | Launch or focus Spotify |
| `5` | ⚙️ Task Manager | Open Task Manager |
| `6` | 📁 File Explorer | Open File Explorer |
| `7` | 🎙️ VoiceMeeter | Launch or focus VoiceMeeter |

</details>

<details>
<summary><strong>🌐 Websites Profile</strong> - Quick Web Access</summary>

| Key | Action | Website |
|-----|--------|----------|
| `1` | 📺 YouTube | Open YouTube |
| `2` | 🔍 Google | Open Google Search |
| `3` | 🎬 9anime | Open 9anime |
| `4` | 🎭 MovieBoxPro | Launch MovieBoxPro app |
| `5` | 💬 Discord Web | Open Discord web app |
| `6` | 🐙 GitHub | Open GitHub |
| `7` | 🟣 Twitch | Open Twitch |
| `8` | 🟢 Kick | Open Kick streaming |
| `0` | 🤖 ChatGPT | Open OpenAI ChatGPT |

</details>

<details>
<summary><strong>🖥️ Hotswap Profile</strong> - Window Management</summary>

| Key | Action | Direction |
|-----|--------|----------|
| `4` | ⬅️ Move Left | Move window to left monitor |
| `6` | ➡️ Move Right | Move window to right monitor |
| `8` | ⬆️ Move Up | Move window to upper monitor |
| `2` | ⬇️ Move Down | Move window to lower monitor |

</details>

<details>
<summary><strong>🖼️ Desktops Profile</strong> - Virtual Desktop Control</summary>

| Key | Action | Function |
|-----|--------|----------|
| `1` | ⬅️ Move Window Left | Move window to previous desktop |
| `3` | ➡️ Move Window Right | Move window to next desktop |
| `4` | 🔄 Switch Left | Switch to previous desktop |
| `6` | 🔄 Switch Right | Switch to next desktop |
| `8` | ➕ New Desktop | Create new virtual desktop |
| `2` | ❌ Close Desktop | Close current virtual desktop |
| `5` | 👁️ View All | Show all virtual desktops |

</details>

---

## 🔧 Installation

### Prerequisites
- Windows 10/11
- [AutoHotkey v1.1+](https://www.autohotkey.com/download/)

### Step-by-Step Installation

1. **Download AutoHotkey**
   ```
   Visit: https://www.autohotkey.com/download/
   Install the latest stable version
   ```

2. **Get Stream Pad**
   ```bash
   git clone https://github.com/OMetaVR/Macro-NUM-pad.git
   # OR download the ZIP from releases
   ```

3. **Configure Application Paths**
   - Open `macropad.ahk` in a text editor
   - Update the application paths at the top of the file:
   ```autohotkey
   global DiscordPath := "C:\Users\YourUsername\AppData\Local\Discord\Update.exe"
   global OperaPath := "C:\Users\YourUsername\AppData\Local\Programs\Opera GX\launcher.exe"
   # ... update other paths as needed
   ```

4. **Run the Script**
   - Double-click `macropad.ahk`
   - Look for the AutoHotkey icon in your system tray

---

## 📖 Usage Guide

### Basic Controls

| Key Combination | Action |
|------------------|--------|
| `ScrollLock` | Toggle Stream Pad on/off |
| `Numpad +` | Next profile |
| `Numpad -` | Previous profile |
| `F1` | Show help for current profile |
| `Ctrl+Alt+S` | Suspend/resume script |

### Smart Features

- **🧠 Browser Detection**: Audio profile automatically detects browsers and switches between media controls and browser navigation
- **📍 Visual Feedback**: Tooltips show current profile and action confirmations
- **🎮 Game Mode**: Special optimizations for gaming (activated via Audio profile)
- **🔄 Persistent Settings**: Your last profile selection is remembered

---

## ⚙️ Customization

### Adding New Applications

1. **Add the application path**:
   ```autohotkey
   global YourAppPath := "C:\Path\To\Your\App.exe"
   ```

2. **Create a new hotkey**:
   ```autohotkey
   Numpad9::
   ActivateOrLaunch("yourapp.exe", YourAppPath)
   return
   ```

### Creating Custom Profiles

You can easily extend the script with additional profiles by:
1. Adding new profile entries to the `profiles` array
2. Creating new `#If` conditions for your profile
3. Defining custom hotkeys for your new profile

### Advanced Configuration

- **Tooltip Duration**: Modify `ShowTooltip()` function parameters
- **Window Movement**: Adjust `MoveActiveWindow()` for custom monitor layouts
- **Game Mode Behavior**: Customize the `XButton2` (Mouse5) functionality

---

## 🎮 Game Mode

Game Mode provides special optimizations for gaming:

- **Activation**: Press `Numpad 3` in Audio profile
- **Mouse5 Integration**: Hold Mouse5 for push-to-talk (Win+Ctrl+Alt+P)
- **Auto-disable**: Automatically disabled when accessing system tray
- **Visual Indicator**: Tooltip confirmation when toggled

---

## 🤝 Contributing

We welcome contributions! Here's how you can help:

1. **🍴 Fork** the repository
2. **🌿 Create** a feature branch (`git checkout -b feature/AmazingFeature`)
3. **💾 Commit** your changes (`git commit -m 'Add some AmazingFeature'`)
4. **📤 Push** to the branch (`git push origin feature/AmazingFeature`)
5. **🔄 Open** a Pull Request

### Development Guidelines

- Follow existing code style and conventions
- Test your changes thoroughly
- Update documentation as needed
- Add comments for complex functionality

---

## 📄 License

This project is licensed under nothing, it's free to use and even claim as your own for all I care, it's not like it's an impressive script lol.

---

## 🙏 Acknowledgements

- **[Takkeshi](https://github.com/Takkeshi)** - Helped push me to create this project
- **[PhoenixAceVFX](https://github.com/PhoenixAceVFX)** - Helped out with small issues
- **AutoHotkey Community** - Inspiration and support

---

<div align="center">

### 💝 Show Your Support

If Stream Pad has improved your workflow, consider:

⭐ **Starring** this repository  
🐛 **Reporting** bugs or **suggesting** features  
🤝 **Contributing** to the codebase  
📢 **Sharing** with friends and colleagues  

**Made with ❤️ by Meta**

</div>
