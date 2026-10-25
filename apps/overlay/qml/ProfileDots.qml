import QtQuick
import StreamPad.Overlay

// Profile position in the header: a dot per profile with the current one as an accent pill.
// Long profile lists collapse to a "3 / 12" counter.
Item {
    id: dots

    readonly property real s: Theme.s
    readonly property int count: Store.profileCount
    readonly property int current: Store.profileIndex
    readonly property bool compact: count > 9
    readonly property real dot: Math.round(6 * s)
    readonly property real pill: Math.round(16 * s)
    readonly property real gap: Math.round(5 * s)

    implicitWidth: compact ? counter.implicitWidth : Math.max(0, (count - 1) * (dot + gap) + pill)
    implicitHeight: compact ? counter.implicitHeight : dot
    width: implicitWidth
    height: implicitHeight
    visible: count > 1 && Controller.geometry.headerRect.height > 0

    Repeater {
        model: dots.compact ? 0 : dots.count

        Rectangle {
            required property int index
            readonly property bool isCurrent: index === dots.current

            x: index * (dots.dot + dots.gap) + (index > dots.current ? dots.pill - dots.dot : 0)
            width: isCurrent ? dots.pill : dots.dot
            height: dots.dot
            radius: height / 2
            color: isCurrent ? Theme.accent : Qt.alpha(Theme.muted, Theme.dark ? 0.38 : 0.32)

            Behavior on x {
                NumberAnimation { duration: 220; easing.type: Easing.OutCubic }
            }
            Behavior on width {
                NumberAnimation { duration: 220; easing.type: Easing.OutCubic }
            }
            Behavior on color {
                ColorAnimation { duration: 220 }
            }
        }
    }

    Text {
        id: counter

        visible: dots.compact
        text: (dots.current + 1) + " / " + dots.count
        color: Theme.muted
        font.pointSize: Theme.pt(11)
        font.weight: Font.Medium
        font.features: { "tnum": 1 }
    }
}
