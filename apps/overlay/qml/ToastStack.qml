import QtQuick
import StreamPad.Overlay

// Up to three toasts, newest nearest the screen edge. They sit beside the panel while it is shown and
// take its place when it is hidden. Keeps the window mapped while any toast is on screen.
Item {
    id: stack

    property bool besidePanel: false
    readonly property int maxVisible: 3

    function push(toast: var) {
        toasts.insert(0, {
            toastId: String(toast.id),
            text: toast.text,
            kind: toast.kind,
            durationMs: toast.durationMs,
            iconId: toast.iconId
        })
    }

    function remove(toastId: string) {
        for (let i = 0; i < toasts.count; ++i) {
            if (toasts.get(i).toastId === toastId) {
                toasts.remove(i)
                return
            }
        }
    }

    ListModel {
        id: toasts

        onCountChanged: Controller.toastsActive = count > 0
    }

    Repeater {
        model: toasts

        Toast {
            besidePanel: stack.besidePanel
            maxVisible: stack.maxVisible
            onDismissed: stack.remove(toastId)
        }
    }
}
