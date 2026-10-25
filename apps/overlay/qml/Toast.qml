import QtQuick
import QtQuick.Effects
import StreamPad.Overlay

// A toast pill (icon + text) that slides out from behind the screen edge and back when it expires.
Item {
    id: toast

    required property int index
    required property string toastId
    required property string text
    required property string kind
    required property int durationMs
    required property string iconId
    property bool besidePanel: false
    property int maxVisible: 3

    signal dismissed()

    readonly property PanelGeometry geo: Controller.geometry
    readonly property real s: Theme.s
    readonly property color kindColor: kind === "error" ? Theme.error : kind === "success" ? Theme.success : Theme.accent
    readonly property real padding: Math.round(14 * s)
    readonly property real iconSize: Math.round(16 * s)
    readonly property point rest: geo.toastPosition(index, width, besidePanel)
    readonly property point hidden: geo.toastSlideVector(width)

    property real enter: 0
    property bool leaving: false
    // Rest position, eased when the stack reflows or moves beside / away from the panel.
    property real restX: rest.x
    property real restY: rest.y

    function leave() {
        if (leaving)
            return
        leaving = true
        enterAnim.stop()
        leaveAnim.duration = Math.round(220 * enter)
        leaveAnim.start()
    }

    width: Math.min(geo.toastMaxWidth, row.implicitWidth + 2 * padding)
    height: geo.toastHeight
    x: restX + hidden.x * (1 - enter)
    y: restY + hidden.y * (1 - enter)
    opacity: enter

    onIndexChanged: if (index >= maxVisible) leave()

    Behavior on restX {
        NumberAnimation { duration: 260; easing.type: Easing.OutCubic }
    }
    Behavior on restY {
        NumberAnimation { duration: 260; easing.type: Easing.OutCubic }
    }

    NumberAnimation {
        id: enterAnim

        target: toast
        property: "enter"
        to: 1
        duration: 280
        easing.type: Easing.OutCubic
        running: Controller.ready && !toast.leaving
    }

    NumberAnimation {
        id: leaveAnim

        target: toast
        property: "enter"
        to: 0
        easing.type: Easing.InCubic
        onFinished: toast.dismissed()
    }

    Timer {
        interval: toast.durationMs
        running: Controller.ready && !toast.leaving
        onTriggered: toast.leave()
    }

    RectangularShadow {
        anchors.fill: pill
        radius: pill.radius
        blur: 16 * toast.s
        offset.y: 3 * toast.s
        color: Qt.rgba(0, 0, 0, Theme.dark ? 0.45 : 0.14)
    }

    Rectangle {
        id: pill

        anchors.fill: parent
        radius: height / 2
        color: toast.kind === "error" ? Theme.tinted(Theme.panel, Theme.error, Theme.dark ? 0.12 : 0.07)
                                      : Theme.panel
        border.width: 1
        border.color: toast.kind === "error" ? Qt.alpha(Theme.error, 0.45) : Theme.border
    }

    Row {
        id: row

        x: toast.padding - Math.round(2 * toast.s)
        anchors.verticalCenter: parent.verticalCenter
        spacing: Math.round(8 * toast.s)

        Image {
            anchors.verticalCenter: parent.verticalCenter
            width: toast.iconSize
            height: toast.iconSize
            sourceSize: Qt.size(toast.iconSize, toast.iconSize)
            source: Theme.icon(toast.iconId !== "" ? toast.iconId
                               : Theme.bundledIcon(toast.kind === "error" ? "circle-alert"
                                                   : toast.kind === "success" ? "circle-check" : "info"),
                               toast.kindColor)
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(implicitWidth, toast.geo.toastMaxWidth - 2 * toast.padding - toast.iconSize - row.spacing)
            text: toast.text
            elide: Text.ElideRight
            maximumLineCount: 1
            color: Theme.text
            font.pointSize: Theme.pt(12.5)
            font.weight: Font.Medium
        }
    }
}
