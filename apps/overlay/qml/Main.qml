import QtQuick
import StreamPad.Overlay

// The overlay window. Its size, screen, anchoring and mapping are managed by Controller; this file
// only drives the slide animation and routes daemon events to the panel and the toast stack.
Window {
    id: window

    color: "transparent"
    title: "StreamPad"
    visible: false

    Panel {
        id: panel
        width: window.width
        height: window.height
    }

    ToastStack {
        id: toasts
        width: window.width
        height: window.height
        besidePanel: Controller.presented
    }

    // Show: OutCubic over animationMs; hide: InCubic over 0.7 × animationMs. Durations scale with the
    // remaining distance so an interrupted slide reverses smoothly from where it is.
    NumberAnimation {
        id: slide
        target: Controller
        property: "progress"
    }

    Connections {
        target: Controller

        function onPresentedChanged() {
            const show = Controller.presented
            const from = Controller.progress
            const duration = Store.settings.animationMs * (show ? 1 - from : 0.7 * from)
            slide.stop()
            slide.to = show ? 1 : 0
            slide.easing.type = show ? Easing.OutCubic : Easing.InCubic
            slide.duration = Math.round(duration)
            slide.start()
        }

        function onReadyChanged() {
            if (!Controller.ready)
                slide.stop()
        }
    }

    Connections {
        target: Store

        function onKeyEvent(key: string, pressed: bool, kind: string) {
            panel.pressKey(key, pressed, kind)
        }

        function onToastReceived(toast: var) {
            toasts.push(toast)
        }
    }
}
