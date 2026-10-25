import QtQuick
import StreamPad.Overlay

// The numpad panel: background shape, header and the key grid. It slides out from behind the screen
// edge with Controller.progress; profile switches slide + crossfade between two pages.
Item {
    id: panel

    readonly property PanelGeometry geo: Controller.geometry
    readonly property real reveal: Controller.progress
    readonly property real s: Theme.s

    property ProfilePage activePage: pageA
    readonly property ProfilePage idlePage: activePage === pageA ? pageB : pageA

    function pressKey(key: string, pressed: bool, kind: string) {
        activePage.pressKey(key, pressed, kind)
    }

    function switchTo(profile: var, direction: int) {
        const incoming = idlePage
        const outgoing = activePage
        switchAnim.stop()
        incoming.profile = profile
        incoming.shift = 18 * s * direction
        incoming.opacity = 0
        outgoingShift.to = -18 * s * direction
        switchAnim.outgoing = outgoing
        switchAnim.incoming = incoming
        activePage = incoming
        switchAnim.start()
    }

    x: geo.slideVector.x * (1 - reveal)
    y: geo.slideVector.y * (1 - reveal)
    // Fades in over the first 40 % of the slide-in (OutCubic reaches 0.78 at t = 0.4).
    opacity: Math.min(1, reveal / 0.78)
    visible: reveal > 0

    PanelBackground {
        x: panel.geo.backgroundRect.x
        y: panel.geo.backgroundRect.y
        width: panel.geo.backgroundRect.width
        height: panel.geo.backgroundRect.height
        geometry: panel.geo
        panelColor: Qt.alpha(Theme.panel, Theme.panelOpacity)
        borderColor: Theme.border
        shadowColor: Theme.shadow
    }

    ProfilePage {
        id: pageA
        live: panel.visible && panel.activePage === pageA
        headerTrailing: dots.visible ? dots.width + 12 * panel.s : 0
    }

    ProfilePage {
        id: pageB
        live: panel.visible && panel.activePage === pageB
        headerTrailing: pageA.headerTrailing
        opacity: 0
    }

    ProfileDots {
        id: dots
        x: panel.geo.headerRect.right - width - Math.round(2 * panel.s)
        y: panel.geo.headerRect.y + Math.round((panel.geo.headerRect.height - height) / 2)
    }

    ParallelAnimation {
        id: switchAnim

        property ProfilePage incoming: pageB
        property ProfilePage outgoing: pageA

        NumberAnimation {
            target: switchAnim.incoming
            property: "shift"
            to: 0
            duration: 220
            easing.type: Easing.OutCubic
        }
        NumberAnimation {
            target: switchAnim.incoming
            property: "opacity"
            to: 1
            duration: 220
            easing.type: Easing.OutCubic
        }
        NumberAnimation {
            id: outgoingShift
            target: switchAnim.outgoing
            property: "shift"
            duration: 220
            easing.type: Easing.OutCubic
        }
        NumberAnimation {
            target: switchAnim.outgoing
            property: "opacity"
            to: 0
            duration: 160
            easing.type: Easing.OutCubic
        }
        onFinished: switchAnim.outgoing.releaseKeys()
    }

    Component.onCompleted: pageA.profile = Store.profile

    Connections {
        target: Store

        function onProfileChanged() {
            const profile = Store.profile
            // Edits to the shown profile, or any change while hidden, apply in place.
            if (profile.id === panel.activePage.profile.id || panel.reveal <= 0) {
                switchAnim.stop()
                panel.activePage.profile = profile
                panel.activePage.shift = 0
                panel.activePage.opacity = 1
                panel.idlePage.opacity = 0
                return
            }
            panel.switchTo(profile, Store.profileDirection)
        }
    }
}
