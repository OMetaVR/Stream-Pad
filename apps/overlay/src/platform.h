#pragma once

namespace streampad::platform {

// Must run before QGuiApplication is constructed. Probes the Wayland compositor for wlr-layer-shell;
// when it is missing (e.g. GNOME) and XWayland is available, selects the xcb platform so the fallback
// window can be positioned. An explicit QT_QPA_PLATFORM is always respected.
void selectQpaPlatform();

// After QGuiApplication exists: whether the window can be a layer-shell surface.
bool layerShellUsable();

} // namespace streampad::platform
