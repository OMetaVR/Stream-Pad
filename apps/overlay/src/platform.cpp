#include "platform.h"

#include <QGuiApplication>
#include <QLoggingCategory>

#ifdef STREAMPAD_HAVE_WAYLAND_CLIENT
#include <wayland-client.h>

#include <cstring>
#endif

Q_LOGGING_CATEGORY(lcPlatform, "streampad.overlay.platform", QtInfoMsg)

namespace streampad::platform {

namespace {

enum class LayerShell { Unknown, Present, Missing };
LayerShell g_layerShell = LayerShell::Unknown;

bool waylandSession()
{
    return !qEnvironmentVariableIsEmpty("WAYLAND_DISPLAY") || qEnvironmentVariable("XDG_SESSION_TYPE") == u"wayland";
}

LayerShell probeLayerShell()
{
#if defined(STREAMPAD_HAVE_WAYLAND_CLIENT) && defined(STREAMPAD_HAVE_LAYERSHELL)
    wl_display *display = wl_display_connect(nullptr);
    if (!display)
        return LayerShell::Unknown;
    bool found = false;
    static const wl_registry_listener listener = {
        .global = [](void *data, wl_registry *, uint32_t, const char *interface, uint32_t) {
            if (std::strcmp(interface, "zwlr_layer_shell_v1") == 0)
                *static_cast<bool *>(data) = true;
        },
        .global_remove = [](void *, wl_registry *, uint32_t) {},
    };
    wl_registry *registry = wl_display_get_registry(display);
    wl_registry_add_listener(registry, &listener, &found);
    const bool ok = wl_display_roundtrip(display) >= 0;
    wl_registry_destroy(registry);
    wl_display_disconnect(display);
    if (!ok)
        return LayerShell::Unknown;
    return found ? LayerShell::Present : LayerShell::Missing;
#elif defined(STREAMPAD_HAVE_LAYERSHELL)
    return LayerShell::Unknown;
#else
    return LayerShell::Missing;
#endif
}

} // namespace

void selectQpaPlatform()
{
    if (!waylandSession())
        return;
    g_layerShell = probeLayerShell();
    if (g_layerShell != LayerShell::Missing || qEnvironmentVariableIsSet("QT_QPA_PLATFORM"))
        return;
    if (!qEnvironmentVariableIsEmpty("DISPLAY")) {
        qCInfo(lcPlatform) << "compositor has no layer-shell; using XWayland so the overlay can be positioned";
        qputenv("QT_QPA_PLATFORM", "xcb");
    }
}

bool layerShellUsable()
{
#ifdef STREAMPAD_HAVE_LAYERSHELL
    return QGuiApplication::platformName() == u"wayland" && g_layerShell != LayerShell::Missing;
#else
    return false;
#endif
}

} // namespace streampad::platform
