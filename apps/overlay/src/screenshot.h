#pragma once

#include <QObject>
#include <QPointer>
#include <QString>
#include <QTimer>

class OverlayController;
class QQuickWindow;

// `--screenshot`: grabs the window content once the panel is presented (after `afterMs`, or once at
// rest when negative), writes a PNG and quits the application with the result.
class ScreenshotTask : public QObject
{
    Q_OBJECT

public:
    ScreenshotTask(QQuickWindow *window, OverlayController *controller, QString path, int afterMs,
                   QObject *parent = nullptr);

private:
    void capture();
    void fail(const QString &reason);

    QPointer<QQuickWindow> m_window;
    OverlayController *m_controller;
    QString m_path;
    QTimer m_captureTimer;
    QTimer m_timeout;
};
