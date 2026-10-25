import QtQuick
import QtQuick.Effects
import StreamPad.Overlay

// A key cover image filling the face with rounded corners. Without `crop` it is centred and cropped to fill
// (PreserveAspectCrop); with `crop` ({x, y, w, h} in 0–1 image coordinates) exactly that region fills the
// face. Animated GIF/WebP covers play only while `playing`.
Item {
    id: cover

    property url source
    property var crop: null
    property bool animated: false
    property bool playing: false
    property real radius: 0

    readonly property bool failed: image.item !== null && (image.item as Image).status === Image.Error

    layer.enabled: true
    layer.effect: MultiEffect {
        maskEnabled: true
        maskSource: mask
        maskThresholdMin: 0.5
        maskSpreadAtMin: 1.0
    }

    // Placement of an image with natural size iw × ih so the crop region covers the face.
    function placement(iw: real, ih: real): rect {
        const c = cover.crop
        if (!c || iw <= 0 || ih <= 0)
            return Qt.rect(0, 0, cover.width, cover.height)
        const scale = Math.max(cover.width / (c.w * iw), cover.height / (c.h * ih))
        const cx = (c.x + c.w / 2) * iw * scale
        const cy = (c.y + c.h / 2) * ih * scale
        return Qt.rect(cover.width / 2 - cx, cover.height / 2 - cy, iw * scale, ih * scale)
    }

    Loader {
        id: image

        readonly property rect box: cover.placement(item?.implicitWidth ?? 0, item?.implicitHeight ?? 0)
        x: box.x
        y: box.y
        width: box.width
        height: box.height
        sourceComponent: cover.animated ? animatedImage : staticImage
    }

    Component {
        id: staticImage

        Image {
            source: cover.source
            fillMode: cover.crop ? Image.Stretch : Image.PreserveAspectCrop
            // Cropped covers need the full image; uncropped ones decode at face size.
            sourceSize: cover.crop ? undefined : Qt.size(cover.width, cover.height)
            smooth: true
            mipmap: !!cover.crop
        }
    }

    Component {
        id: animatedImage

        AnimatedImage {
            source: cover.source
            fillMode: cover.crop ? Image.Stretch : Image.PreserveAspectCrop
            playing: cover.playing
            // Decoding on the fly keeps memory flat for long or large GIFs.
            cache: false
            smooth: true
        }
    }

    Item {
        id: mask

        anchors.fill: parent
        layer.enabled: true
        visible: false

        Rectangle {
            anchors.fill: parent
            radius: cover.radius
            color: "black"
        }
    }
}
