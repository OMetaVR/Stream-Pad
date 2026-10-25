import QtQuick
import StreamPad.Overlay

// One profile's header (icon + name) and its 17 keycaps, laid out over the panel body.
Item {
    id: page

    property var profile: ({ id: "", name: "", iconId: "", keys: ({}) })
    // Vertical offset used by the profile-switch slide.
    property real shift: 0
    // Visible and current: animated covers play only then.
    property bool live: false
    // Room kept free at the right end of the header (for the profile dots).
    property real headerTrailing: 0

    readonly property PanelGeometry geo: Controller.geometry
    readonly property real s: Theme.s
    readonly property var keyIndex: {
        const index = {}
        const layout = Store.keyLayout
        for (let i = 0; i < layout.length; ++i)
            index[layout[i].id] = i
        return index
    }

    function pressKey(key: string, pressed: bool, kind: string) {
        const cap = keys.itemAt(keyIndex[key] ?? -1) as KeyCap
        if (cap)
            cap.setPressed(pressed, kind)
    }

    function releaseKeys() {
        for (let i = 0; i < keys.count; ++i)
            (keys.itemAt(i) as KeyCap).reset()
    }

    x: geo.panelRect.x
    y: geo.panelRect.y
    width: geo.panelRect.width
    height: geo.panelRect.height
    transform: Translate { y: page.shift }

    Item {
        id: header

        x: page.geo.headerRect.x - page.geo.panelRect.x
        y: page.geo.headerRect.y - page.geo.panelRect.y
        width: page.geo.headerRect.width - page.headerTrailing
        height: page.geo.headerRect.height
        visible: height > 0

        Image {
            id: profileIcon

            readonly property real size: Math.round(16 * page.s)

            x: Math.round(2 * page.s)
            anchors.verticalCenter: parent.verticalCenter
            width: size
            height: size
            sourceSize: Qt.size(size, size)
            source: Theme.icon(page.profile.iconId, Theme.accent)
            visible: status === Image.Ready
        }

        Text {
            x: profileIcon.visible ? profileIcon.x + profileIcon.width + Math.round(8 * page.s) : Math.round(2 * page.s)
            width: header.width - x
            anchors.verticalCenter: parent.verticalCenter
            text: page.profile.name
            color: Theme.text
            elide: Text.ElideRight
            font.pointSize: Theme.pt(13)
            font.weight: Font.DemiBold
            font.letterSpacing: -0.1 * page.s
        }
    }

    Item {
        id: grid

        readonly property real pitch: page.geo.keyUnit + page.geo.keyGap

        x: page.geo.gridRect.x - page.geo.panelRect.x
        y: page.geo.gridRect.y - page.geo.panelRect.y

        Repeater {
            id: keys

            model: Store.keyLayout

            KeyCap {
                required property var modelData

                keyId: modelData.id
                keyData: page.profile.keys[modelData.id] ?? ({})
                live: page.live
                x: modelData.col * grid.pitch
                y: modelData.row * grid.pitch
                width: modelData.colSpan * grid.pitch - page.geo.keyGap
                height: modelData.rowSpan * grid.pitch - page.geo.keyGap
            }
        }
    }
}
