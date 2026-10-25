#include "screenshot.h"

#include "controller.h"

#include <QCoreApplication>
#include <QImage>
#include <QQuickWindow>

#include <cstdio>

namespace {

// Time for in-panel transitions (key glow, toast slide) to finish after the panel comes to rest.
constexpr int kSettleMs = 450;
constexpr int kTimeoutMs = 15000;

} // namespace

ScreenshotTask::ScreenshotTask(QQuickWindow *window, OverlayController *controller, QString path, int afterMs,
                               QObject *parent)
    : QObject(parent), m_window(window), m_controller(controller), m_path(std::move(path))
{
    m_captureTimer.setSingleShot(true);
    connect(&m_captureTimer, &QTimer::timeout, this, &ScreenshotTask::capture);
    m_timeout.setSingleShot(true);
    connect(&m_timeout, &QTimer::timeout, this, [this] { fail(QStringLiteral("timed out waiting for the panel")); });
    m_timeout.start(kTimeoutMs);

    if (afterMs >= 0) {
        connect(controller, &OverlayController::presentedChanged, this, [this, afterMs] {
            if (m_controller->presented() && !m_captureTimer.isActive())
                m_captureTimer.start(afterMs);
        });
    } else {
        connect(controller, &OverlayController::progressChanged, this, [this] {
            if (m_controller->progress() >= 1 && !m_captureTimer.isActive())
                m_captureTimer.start(kSettleMs);
        });
    }
}

void ScreenshotTask::capture()
{
    if (!m_window)
        return fail(QStringLiteral("window is gone"));
    const QImage image = m_window->grabWindow();
    if (image.isNull())
        return fail(QStringLiteral("could not grab the window"));
    if (!image.save(m_path, "PNG"))
        return fail(QStringLiteral("could not write %1").arg(m_path));
    std::printf("%s %dx%d\n", qPrintable(m_path), image.width(), image.height());
    std::fflush(stdout);
    QCoreApplication::exit(0);
}

void ScreenshotTask::fail(const QString &reason)
{
    std::fprintf(stderr, "streampad-overlay: screenshot failed: %s\n", qPrintable(reason));
    QCoreApplication::exit(1);
}
