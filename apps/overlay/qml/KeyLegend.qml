import QtQuick
import StreamPad.Overlay

// Keycap legend: the main glyph with the secondary one ("Home", "←") inline after it. A secondary
// legend that does not fit inline stacks below the main one when `allowStack`, otherwise it is dropped
// so it never collides with the key's icon. On covers the legend sits on a dark scrim chip.
Item {
    id: legend

    property string main
    property string sub
    property bool allowStack: false
    property bool onCover: false
    property real maxWidth: 0

    readonly property real s: Theme.s
    readonly property real gap: Math.round(3 * s)
    readonly property real padX: onCover ? Math.round(5 * s) : 0
    readonly property real padY: onCover ? Math.round(2 * s) : 0
    readonly property bool fitsInline: mainText.implicitWidth + gap + subText.implicitWidth <= maxWidth - 2 * padX
    readonly property bool showSub: sub !== "" && (fitsInline || allowStack)
    readonly property bool stacked: showSub && !fitsInline

    implicitWidth: 2 * padX + (!showSub ? mainText.implicitWidth
                                        : stacked ? Math.max(mainText.implicitWidth, subText.implicitWidth)
                                                  : mainText.implicitWidth + gap + subText.implicitWidth)
    implicitHeight: padY + (stacked ? subText.y + subText.height : mainText.y + mainText.height)
    width: implicitWidth
    height: implicitHeight

    Rectangle {
        anchors.fill: parent
        visible: legend.onCover
        radius: Math.round(6 * legend.s)
        color: Qt.rgba(0.04, 0.04, 0.06, 0.62)
    }

    Text {
        id: mainText

        x: legend.padX
        y: legend.padY
        text: legend.main
        color: legend.onCover ? "#ffffff" : Theme.text
        font.pointSize: Theme.pt(14)
        font.weight: Font.DemiBold
        lineHeightMode: Text.FixedHeight
        lineHeight: Math.round(16 * legend.s)
    }

    Text {
        id: subText

        visible: legend.showSub
        x: legend.stacked ? mainText.x : mainText.x + mainText.implicitWidth + legend.gap
        y: legend.stacked ? mainText.y + mainText.height - Math.round(2 * legend.s)
                          : mainText.y + mainText.baselineOffset - baselineOffset
        text: legend.sub
        color: legend.onCover ? Qt.rgba(1, 1, 1, 0.78) : Theme.muted
        font.pointSize: Theme.pt(10)
        font.weight: Font.Medium
        lineHeightMode: Text.FixedHeight
        lineHeight: Math.round(12 * legend.s)
    }
}
