pragma Singleton
import QtQuick
import StreamPad.Overlay

// Colour tokens (docs/SPEC.md §8 "Theme") and small helpers shared by the overlay components.
QtObject {
    readonly property bool dark: Store.settings.theme !== "light"
    readonly property real s: Controller.geometry.scale
    readonly property color accent: Store.accent

    readonly property color panel: dark ? "#121318" : "#f6f6f8"
    readonly property real panelOpacity: Store.settings.opacity
    readonly property color border: dark ? Qt.rgba(1, 1, 1, 0.08) : Qt.rgba(0, 0, 0, 0.08)
    readonly property color keycap: dark ? "#1f2128" : "#ffffff"
    readonly property color skirt: dark ? "#14151a" : "#d9dae0"
    readonly property color highlight: dark ? Qt.rgba(1, 1, 1, 0.06) : Qt.rgba(1, 1, 1, 1)
    readonly property color text: dark ? "#e9e9ef" : "#17181c"
    readonly property color muted: dark ? "#8b8d98" : "#6b6e7a"
    readonly property color shadow: dark ? Qt.rgba(0, 0, 0, 0.6) : Qt.rgba(0.08, 0.09, 0.16, 0.2)
    readonly property color success: dark ? "#3ecf8e" : "#16a06a"
    readonly property color error: dark ? "#f2555a" : "#dc3a40"

    // Font size for a design size in logical px. Points allow fractional sizes (10.5 px labels);
    // the app runs with AA_Use96Dpi, so 1 pt is exactly 4/3 px.
    function pt(px: real): real {
        return px * s * 0.75
    }

    // Accent mixed into a base colour, e.g. for key tints and the error toast.
    function tinted(base: color, tint: color, amount: real): color {
        return Qt.tint(base, Qt.alpha(tint, amount))
    }

    // image://icon URL for an icon id (percent-encoded path). Lucide SVGs are tinted with `tint`;
    // pass null to draw the file as-is.
    function icon(iconId: string, tint: var): string {
        if (!iconId)
            return ""
        const hex = tint ? Qt.color(tint).toString().slice(-6) : "-"
        return "image://icon/" + hex + "/" + iconId
    }

    function bundledIcon(name: string): string {
        return encodeURIComponent(":/qt/qml/StreamPad/Overlay/icons/" + name + ".svg")
    }
}
