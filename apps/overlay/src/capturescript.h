#pragma once

#include <QString>
#include <QVector>

namespace streampad {

struct CaptureEvent {
    enum class Type { Visible, Key, Toast, Profile };

    int frame = 0;
    Type type = Type::Visible;
    bool flag = false; // visible: value; key: pressed
    QString text;      // key: KeyId; toast: text; profile: absolute state path
    QString kind;      // key: macro|global|empty; toast: info|success|error
    int number = 0;    // toast: durationMs; profile: direction (+1 / -1)
};

// The `--capture` script (docs in apps/overlay/README.md): frame rate, frame count, the initial
// DaemonState file and the timed events. Relative paths are resolved against the script's directory.
struct CaptureScript {
    int fps = 30;
    int frames = 0;
    QString statePath;
    QVector<CaptureEvent> events; // sorted by frame, script order within a frame

    // Returns false and sets `error` if the file is unreadable or invalid.
    bool load(const QString &path, QString *error);
};

} // namespace streampad
