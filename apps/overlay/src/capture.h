#pragma once

#include "capturescript.h"

#include <QJsonObject>
#include <QObject>
#include <QPointer>
#include <QSize>
#include <QString>

class OverlayController;
class OverlayStore;
class QQuickWindow;
class SteppedClock;

// `--capture`: plays a script against the real overlay on a fixed-step clock and writes every frame as an
// RGBA PNG plus manifest.json. Frame n shows the scene at n / fps seconds: the clock is advanced first,
// then the frame's events are delivered, the window settles, and the result is grabbed. Events are the
// ones the daemon would send (state, profile, key, toast), so the real code paths and animations run.
class CaptureTask : public QObject
{
    Q_OBJECT

public:
    CaptureTask(QQuickWindow *window, OverlayController *controller, OverlayStore *store, SteppedClock *clock,
                streampad::CaptureScript script, QString outDir, QObject *parent = nullptr);

    // Renders all frames, then quits the application with the result (0 = success).
    void run();

private:
    bool applyState(const QString &path);
    bool deliver(const streampad::CaptureEvent &event);
    void settle();
    void waitForFrame();
    QImage grabFrame() const;
    bool writeManifest() const;
    QPoint cornerAnchor() const;
    bool fail(const QString &reason);

    QPointer<QQuickWindow> m_window;
    OverlayController *m_controller;
    OverlayStore *m_store;
    SteppedClock *m_clock;
    streampad::CaptureScript m_script;
    QString m_outDir;
    QJsonObject m_state;
    bool m_visible = false;
    int m_toastId = 0;
    QSize m_size;
};
