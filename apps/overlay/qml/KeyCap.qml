import QtQuick
import QtQuick.Effects
import StreamPad.Overlay

// One keycap (docs/SPEC.md §8 "Key face"): a darker skirt under a face with a 1 px top highlight,
// showing either the cover image or icon + label, plus the legend. Presses depress the face by 2 px
// and light an accent glow (fade in 60 ms, out 260 ms; subtler on empty keys).
Item {
    id: cap

    required property string keyId
    property var keyData: ({})
    property bool live: false

    readonly property real s: Theme.s
    readonly property real radius: Controller.geometry.keyRadius
    readonly property real skirtHeight: Math.round(3 * s)
    readonly property bool isEmpty: (keyData.kind ?? "empty") === "empty"
    readonly property bool isGlobal: keyData.kind === "global"
    readonly property bool hasTint: !!keyData.color
    readonly property bool coverShown: !!keyData.coverUrl && !(coverLoader.item?.failed ?? false)
    readonly property bool showLegends: Store.settings.showLegends
    readonly property color baseColor: hasTint ? Theme.tinted(Theme.keycap, keyData.color, Theme.dark ? 0.2 : 0.14)
                                               : Theme.keycap
    readonly property color skirtColor: hasTint ? Theme.tinted(Theme.skirt, keyData.color, Theme.dark ? 0.28 : 0.3)
                                                : Theme.skirt
    readonly property color faceColor: Theme.tinted(baseColor, Theme.accent, 0.1 * glow * glowStrength)

    property real depress: 0
    property real glow: 0
    property real glowStrength: 1
    property bool held: false

    function setPressed(pressed: bool, kind: string) {
        held = pressed
        if (pressed) {
            glowStrength = kind === "empty" || isEmpty ? 0.45 : 1
            releaseAnim.stop()
            pressAnim.restart()
        } else if (!pressAnim.running) {
            releaseAnim.restart()
        }
    }

    function reset() {
        pressAnim.stop()
        releaseAnim.stop()
        held = false
        depress = 0
        glow = 0
    }

    ParallelAnimation {
        id: pressAnim

        NumberAnimation { target: cap; property: "depress"; to: 1; duration: 60; easing.type: Easing.OutCubic }
        NumberAnimation { target: cap; property: "glow"; to: 1; duration: 60; easing.type: Easing.OutCubic }
        // A quick tap still shows the full press before letting go.
        onFinished: if (!cap.held) releaseAnim.restart()
    }

    ParallelAnimation {
        id: releaseAnim

        NumberAnimation { target: cap; property: "depress"; to: 0; duration: 140; easing.type: Easing.OutCubic }
        NumberAnimation { target: cap; property: "glow"; to: 0; duration: 260; easing.type: Easing.OutQuad }
    }

    RectangularShadow {
        anchors.fill: parent
        radius: cap.radius
        blur: 18 * cap.s
        spread: 2 * cap.s
        color: Qt.alpha(Theme.accent, Theme.dark ? 0.85 : 0.5)
        opacity: cap.glow * cap.glowStrength
        visible: opacity > 0
    }

    Rectangle {
        anchors.fill: parent
        radius: cap.radius
        color: cap.skirtColor
    }

    Item {
        id: face

        width: parent.width
        height: parent.height - cap.skirtHeight
        y: Math.round(2 * cap.s) * cap.depress

        Rectangle {
            anchors.fill: parent
            radius: cap.radius
            color: Qt.tint(cap.faceColor, Theme.highlight)
        }

        Rectangle {
            anchors.fill: parent
            anchors.topMargin: 1
            radius: cap.radius
            color: cap.faceColor
        }

        Loader {
            id: coverLoader

            anchors.fill: parent
            active: !!cap.keyData.coverUrl

            sourceComponent: KeyCover {
                source: cap.keyData.coverUrl
                crop: cap.keyData.coverCrop ?? null
                animated: cap.keyData.coverAnimated
                playing: cap.live
                radius: cap.radius
            }
        }

        Item {
            id: content

            readonly property bool hasIcon: !!cap.keyData.iconId
            readonly property bool hasLabel: label.text !== ""
            readonly property real spacing: Math.round(2 * cap.s)
            readonly property real lineHeight: Math.round(12 * cap.s)
            readonly property real fullIcon: Math.round(24 * cap.s)
            // On wide keys the icon column clears the legend and may use the whole face height;
            // otherwise the icon and label sit below the legend row.
            readonly property bool clearOfLegend: !legend.visible
                || (face.width - fullIcon) / 2 >= legend.x + legend.width + Math.round(4 * cap.s)
            readonly property real areaTop: clearOfLegend ? Math.round(5 * cap.s)
                                                          : legend.y + legend.height - Math.round(2 * cap.s)
            readonly property real areaHeight: face.height - Math.round(3 * cap.s) - areaTop
            readonly property real labelWidth: face.width - Math.round(8 * cap.s)
            // A label that needs two lines gets them when the icon can shrink to make room.
            readonly property bool twoLines: hasLabel && labelMetrics.advanceWidth > labelWidth
                && areaHeight - 2 * lineHeight - spacing >= (hasIcon ? Math.round(18 * cap.s) : 0)
            readonly property int maxLines: hasLabel && (twoLines || !hasIcon) ? 2 : 1
            readonly property real iconSize: !hasIcon ? 0
                : Math.max(Math.round(16 * cap.s),
                           Math.min(fullIcon, areaHeight - (hasLabel ? spacing + (twoLines ? 2 : 1) * lineHeight : 0)))

            TextMetrics {
                id: labelMetrics

                font: label.font
                text: label.text
            }

            anchors.fill: parent
            visible: !cap.coverShown && (hasIcon || hasLabel)

            Column {
                id: stack

                anchors.horizontalCenter: parent.horizontalCenter
                // Centred in the free area; an icon on its own sits slightly above the middle.
                y: Math.round(content.hasLabel || !content.clearOfLegend
                              ? content.areaTop + Math.max(0, (content.areaHeight - height) / 2)
                              : Math.max(content.areaTop, (face.height - height) / 2 - 1.5 * cap.s))
                spacing: content.spacing

                Image {
                    id: icon

                    anchors.horizontalCenter: parent.horizontalCenter
                    width: content.iconSize
                    height: content.iconSize
                    sourceSize: Qt.size(content.iconSize, content.iconSize)
                    source: Theme.icon(cap.keyData.iconId ?? "",
                                       cap.keyData.iconTint ? (cap.isGlobal ? Theme.accent : Theme.text) : null)
                    visible: content.hasIcon
                }

                Text {
                    id: label

                    width: content.labelWidth
                    visible: text !== ""
                    text: Store.settings.showLabels ? (cap.keyData.label ?? "") : ""
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: content.maxLines > 1 ? Text.Wrap : Text.NoWrap
                    maximumLineCount: content.maxLines
                    elide: Text.ElideRight
                    // Single-line labels shrink a little before they elide.
                    fontSizeMode: content.maxLines > 1 ? Text.FixedSize : Text.HorizontalFit
                    minimumPointSize: Theme.pt(9)
                    lineHeightMode: Text.FixedHeight
                    lineHeight: content.lineHeight
                    color: Qt.alpha(Theme.text, Theme.dark ? 0.78 : 0.82)
                    font.pointSize: Theme.pt(10.5)
                    font.weight: Font.Medium
                }
            }
        }

        KeyLegend {
            id: legend

            x: Math.round((cap.coverShown ? 4 : 7) * cap.s)
            y: Math.round(4 * cap.s)
            visible: cap.showLegends
            opacity: cap.isEmpty ? 0.45 : 1
            main: cap.keyData.legendMain ?? ""
            sub: Store.settings.legendStyle === "full" ? (cap.keyData.legendSub ?? "") : ""
            onCover: cap.coverShown
            allowStack: !content.visible && !cap.coverShown
            maxWidth: face.width - 2 * x
        }
    }

    Rectangle {
        readonly property real inset: Math.round(2 * cap.s)

        x: -inset
        y: -inset
        width: parent.width + 2 * inset
        height: parent.height + 2 * inset
        radius: cap.radius + inset
        color: "transparent"
        border.width: 1.5 * cap.s
        border.color: Theme.accent
        opacity: cap.glow * cap.glowStrength
        visible: opacity > 0
    }
}
