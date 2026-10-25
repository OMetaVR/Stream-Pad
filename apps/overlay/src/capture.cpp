#include "capture.h"

#include "controller.h"
#include "mocksource.h"
#include "steppedclock.h"
#include "store.h"

#include <QCoreApplication>
#include <QDir>
#include <QEventLoop>
#include <QFile>
#include <QImage>
#include <QJsonDocument>
#include <QQuickWindow>
#include <QTimer>

#include <cmath>
#include <cstdio>

using namespace Qt::StringLiterals;
using Edge = PanelGeometry::Edge;
using Align = PanelGeometry::Align;

namespace {

// Each round processes queued work and renders one frame. The window lifecycle chains (map → first
// frame → ready → present, and empty frame → unmap) need up to three of them.
constexpr int kSettleRounds = 4;
constexpr int kFrameTimeoutMs = 5000;

} // namespace

CaptureTask::CaptureTask(QQuickWindow *window, OverlayController *controller, OverlayStore *store,
                         SteppedClock *clock, streampad::CaptureScript script, QString outDir, QObject *parent)
    : QObject(parent), m_window(window), m_controller(controller), m_store(store), m_clock(clock),
      m_script(std::move(script)), m_outDir(std::move(outDir))
{
}

bool CaptureTask::fail(const QString &reason)
{
    std::fprintf(stderr, "streampad-overlay: capture failed: %s\n", qPrintable(reason));
    QCoreApplication::exit(1);
    return false;
}

// Loads a DaemonState fixture (same format and asset resolution as --mock) and feeds it to the store.
// `overlay.visible` is owned by the script's visible events, so it is overridden here.
bool CaptureTask::applyState(const QString &path)
{
    bool loaded = false;
    {
        MockSource source(path);
        QObject::connect(&source, &MockSource::stateReceived, this, [&](const QJsonObject &state) {
            m_state = state;
            loaded = true;
        });
        source.load();
    }
    if (!loaded)
        return fail(u"cannot load state %1"_s.arg(path));
    QJsonObject overlay = m_state.value(u"overlay").toObject();
    overlay.insert(u"visible"_s, m_visible);
    m_state.insert(u"overlay"_s, overlay);
    m_store->applyState(m_state);
    return true;
}

bool CaptureTask::deliver(const streampad::CaptureEvent &event)
{
    using Type = streampad::CaptureEvent::Type;
    switch (event.type) {
    case Type::Visible: {
        m_visible = event.flag;
        QJsonObject overlay = m_state.value(u"overlay").toObject();
        overlay.insert(u"visible"_s, m_visible);
        m_state.insert(u"overlay"_s, overlay);
        m_store->applyState(m_state);
        break;
    }
    case Type::Key:
        m_store->applyEvent(u"key"_s, {{u"key"_s, event.text}, {u"pressed"_s, event.flag}, {u"kind"_s, event.kind}});
        break;
    case Type::Toast:
        m_store->applyEvent(u"toast"_s, {{u"id"_s, ++m_toastId},
                                         {u"text"_s, event.text},
                                         {u"kind"_s, event.kind},
                                         {u"durationMs"_s, event.number}});
        break;
    case Type::Profile:
        // The daemon announces the direction first and sends the new state right after.
        m_store->applyEvent(u"profile"_s, {{u"direction"_s, event.number}});
        return applyState(event.text);
    }
    return true;
}

// Waits for the next rendered frame (the window lifecycle is driven by frameSwapped).
void CaptureTask::waitForFrame()
{
    QEventLoop loop;
    QTimer timeout;
    timeout.setSingleShot(true);
    connect(&timeout, &QTimer::timeout, &loop, &QEventLoop::quit);
    connect(m_window, &QQuickWindow::frameSwapped, &loop, &QEventLoop::quit);
    timeout.start(kFrameTimeoutMs);
    m_window->update();
    loop.exec();
}

// Lets everything the last input triggered run to completion without advancing the clock.
void CaptureTask::settle()
{
    for (int round = 0; round < kSettleRounds; ++round) {
        QCoreApplication::sendPostedEvents();
        QCoreApplication::processEvents();
        if (m_window->isVisible())
            waitForFrame();
    }
    QCoreApplication::sendPostedEvents();
    QCoreApplication::processEvents();
}

// The window content, or a transparent frame while the window is unmapped (nothing to show).
QImage CaptureTask::grabFrame() const
{
    QImage image;
    if (m_window->isVisible())
        image = m_window->grabWindow();
    if (image.isNull()) {
        image = QImage(m_size, QImage::Format_RGBA8888);
        image.fill(Qt::transparent);
    }
    return image.convertToFormat(QImage::Format_RGBA8888);
}

// Where the monitor's corner lies in window coordinates. The panel sits flush in that corner, so its
// fillets meet both screen edges there. Beyond the window by `alignedMargin` when the layout has an offset.
QPoint CaptureTask::cornerAnchor() const
{
    const PanelGeometry *g = m_controller->geometry();
    const bool horizontal = g->edge() == Edge::Bottom || g->edge() == Edge::Top;
    const int margin = g->alignedMargin();
    const bool atEnd = g->align() == Align::End;
    const int along = horizontal ? m_size.width() : m_size.height();
    const int alongPos = atEnd ? along + margin : -margin;
    if (horizontal)
        return {alongPos, g->edge() == Edge::Bottom ? m_size.height() : 0};
    return {g->edge() == Edge::Right ? m_size.width() : 0, alongPos};
}

bool CaptureTask::writeManifest() const
{
    const QPoint anchor = cornerAnchor();
    const QJsonObject manifest{
        {u"fps"_s, m_script.fps},
        {u"frames"_s, m_script.frames},
        {u"width"_s, m_size.width()},
        {u"height"_s, m_size.height()},
        {u"anchor"_s, QJsonObject{{u"x"_s, anchor.x()}, {u"y"_s, anchor.y()}}},
    };
    QFile file(QDir(m_outDir).filePath(u"manifest.json"_s));
    return file.open(QIODevice::WriteOnly) && file.write(QJsonDocument(manifest).toJson()) > 0;
}

void CaptureTask::run()
{
    if (!QDir().mkpath(m_outDir))
        return void(fail(u"cannot create %1"_s.arg(m_outDir)));
    m_store->setConnected(true);
    if (!applyState(m_script.statePath))
        return;
    settle();
    m_size = m_controller->geometry()->windowSize();

    qsizetype next = 0;
    for (int frame = 0; frame < m_script.frames; ++frame) {
        m_clock->advanceTo(std::llround(frame * 1000.0 / m_script.fps));
        for (; next < m_script.events.size() && m_script.events[next].frame == frame; ++next) {
            if (!deliver(m_script.events[next]))
                return;
        }
        settle();

        const QImage image = grabFrame();
        const QString path = QDir(m_outDir).filePath(u"%1.png"_s.arg(frame, 4, 10, QChar(u'0')));
        if (image.size() != m_size)
            return void(fail(u"frame %1 is %2x%3, expected %4x%5 (the window size must not change)"_s
                                 .arg(frame).arg(image.width()).arg(image.height()).arg(m_size.width()).arg(m_size.height())));
        if (!image.save(path, "PNG"))
            return void(fail(u"cannot write %1"_s.arg(path)));
    }
    if (!writeManifest())
        return void(fail(u"cannot write manifest.json in %1"_s.arg(m_outDir)));
    std::printf("%s: %d frames, %dx%d\n", qPrintable(m_outDir), m_script.frames, m_size.width(), m_size.height());
    std::fflush(stdout);
    QCoreApplication::exit(0);
}
