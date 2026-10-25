#include "controller.h"

#include "platform.h"
#include "store.h"

#include <QCursor>
#include <QGuiApplication>
#include <QJSEngine>
#include <QLoggingCategory>
#include <QQuickWindow>
#include <QScreen>

#include <algorithm>

#ifdef STREAMPAD_HAVE_LAYERSHELL
#include <LayerShellQt/Window>
#endif
#ifdef STREAMPAD_HAVE_KWINDOWSYSTEM
#include <KWindowEffects>
#endif

using namespace Qt::StringLiterals;

Q_LOGGING_CATEGORY(lcWindow, "streampad.overlay.window", QtInfoMsg)

namespace {

using Edge = PanelGeometry::Edge;
using Align = PanelGeometry::Align;

OverlayController *s_instance = nullptr;

bool horizontalEdge(Edge edge)
{
    return edge == Edge::Bottom || edge == Edge::Top;
}

QScreen *screenNamed(const QString &name)
{
    if (name.isEmpty())
        return nullptr;
    const QList<QScreen *> screens = QGuiApplication::screens();
    const auto it = std::find_if(screens.begin(), screens.end(), [&](QScreen *s) { return s->name() == name; });
    return it == screens.end() ? nullptr : *it;
}

#ifdef STREAMPAD_HAVE_LAYERSHELL
LayerShellQt::Window::Anchors anchorsFor(const PanelGeometry &g)
{
    using LS = LayerShellQt::Window;
    LS::Anchors anchors;
    switch (g.edge()) {
    case Edge::Bottom:
        anchors = LS::AnchorBottom;
        break;
    case Edge::Top:
        anchors = LS::AnchorTop;
        break;
    case Edge::Left:
        anchors = LS::AnchorLeft;
        break;
    case Edge::Right:
        anchors = LS::AnchorRight;
        break;
    }
    const bool horizontal = horizontalEdge(g.edge());
    if (g.align() == Align::Start)
        anchors |= horizontal ? LS::AnchorLeft : LS::AnchorTop;
    else if (g.align() == Align::End)
        anchors |= horizontal ? LS::AnchorRight : LS::AnchorBottom;
    return anchors;
}

QMargins marginsFor(const PanelGeometry &g)
{
    QMargins margins;
    const bool horizontal = horizontalEdge(g.edge());
    if (g.align() == Align::Start)
        horizontal ? margins.setLeft(g.alignedMargin()) : margins.setTop(g.alignedMargin());
    else if (g.align() == Align::End)
        horizontal ? margins.setRight(g.alignedMargin()) : margins.setBottom(g.alignedMargin());
    return margins;
}
#endif

// Window position without layer-shell, mirroring the layer-shell anchoring on `screen`.
QPoint fallbackPosition(const QRect &screen, const PanelGeometry &g)
{
    const QSize size = g.windowSize();
    const auto along = [&](int start, int length, int extent) {
        switch (g.align()) {
        case Align::Start:
            return start + g.alignedMargin();
        case Align::End:
            return start + length - extent - g.alignedMargin();
        case Align::Center:
            break;
        }
        return start + (length - extent) / 2;
    };
    if (horizontalEdge(g.edge())) {
        const int y = g.edge() == Edge::Top ? screen.top() : screen.top() + screen.height() - size.height();
        return {along(screen.left(), screen.width(), size.width()), y};
    }
    const int x = g.edge() == Edge::Left ? screen.left() : screen.left() + screen.width() - size.width();
    return {x, along(screen.top(), screen.height(), size.height())};
}

} // namespace

OverlayController::OverlayController(OverlayStore *store, QObject *parent) : QObject(parent), m_store(store)
{
    Q_ASSERT(!s_instance);
    s_instance = this;
    m_geometry.setLayout(store->settings());

    connect(store, &OverlayStore::settingsChanged, this, [this] {
        m_geometry.setLayout(m_store->settings());
        applyBlur();
    });
    connect(&m_geometry, &PanelGeometry::changed, this, [this] {
        if (!m_mapped)
            return;
        // Live edits (e.g. the editor's overlay preview) re-place the visible window in place.
        applyPlacement();
        applyInputRegion();
        applyBlur();
    });
    connect(store, &OverlayStore::visibleChanged, this, &OverlayController::scheduleSync);
}

OverlayController::~OverlayController()
{
    s_instance = nullptr;
}

OverlayController *OverlayController::create(QQmlEngine *, QJSEngine *engine)
{
    Q_ASSERT(s_instance && engine->thread() == s_instance->thread());
    QJSEngine::setObjectOwnership(s_instance, QJSEngine::CppOwnership);
    return s_instance;
}

void OverlayController::attach(QQuickWindow *window)
{
    m_window = window;
    m_layerShell = streampad::platform::layerShellUsable();
#ifdef STREAMPAD_HAVE_LAYERSHELL
    if (m_layerShell) {
        // Must be configured before the platform window exists (first show).
        auto *surface = LayerShellQt::Window::get(window);
        surface->setLayer(LayerShellQt::Window::LayerOverlay);
        surface->setKeyboardInteractivity(LayerShellQt::Window::KeyboardInteractivityNone);
        surface->setExclusiveZone(-1);
        surface->setScope(QStringLiteral("streampad"));
        surface->setActivateOnShow(false);
    }
#endif
    if (!m_layerShell) {
        Qt::WindowFlags flags = Qt::FramelessWindowHint | Qt::NoDropShadowWindowHint;
        if (QGuiApplication::platformName() != u"offscreen")
            flags |= Qt::Tool | Qt::WindowStaysOnTopHint | Qt::WindowDoesNotAcceptFocus | Qt::WindowTransparentForInput;
        window->setFlags(flags);
    }
    qCInfo(lcWindow) << "platform" << QGuiApplication::platformName() << (m_layerShell ? "with layer-shell" : "without layer-shell");

    window->resize(m_geometry.windowSize());
    connect(window, &QWindow::visibleChanged, this, [this](bool visible) {
        if (!visible)
            onWindowHidden();
    });
    // The window is torn down on quit; that is not a dismissal to recover from.
    connect(qApp, &QCoreApplication::aboutToQuit, this, [this] {
        if (m_window)
            disconnect(m_window, nullptr, this, nullptr);
    });
    sync();
}

void OverlayController::setForceVisible(bool force)
{
    m_forceVisible = force;
    scheduleSync();
}

bool OverlayController::wantPanel() const
{
    return m_forceVisible || m_store->visible();
}

void OverlayController::scheduleSync()
{
    if (m_syncQueued)
        return;
    m_syncQueued = true;
    QMetaObject::invokeMethod(this, &OverlayController::sync, Qt::QueuedConnection);
}

void OverlayController::sync()
{
    m_syncQueued = false;
    if (!m_window)
        return;

    const bool panel = wantPanel();
    if (panel && m_mapped && !m_mappedForPanel) {
        // Mapped only for toasts: re-pick the screen now that the panel is about to appear.
        const QString output = m_store->output();
        if (output.isEmpty() || output != m_mappedOutput)
            unmap();
        else
            m_mappedForPanel = true;
    }

    const bool needed = panel || m_toastsActive || m_progress > 0;
    if (needed && !m_mapped)
        map();
    if (!m_ready)
        return; // presentation starts once the first (off-screen) frame is out

    setPresented(panel);
    if (!panel && m_progress <= 0) {
        m_mappedForPanel = false;
        if (!m_toastsActive)
            unmapAfterEmptyFrame();
    }
}

void OverlayController::unmapAfterEmptyFrame()
{
    // Unmap only once a frame without the panel has reached the compositor; unmapping straight away would
    // let the close animation replay the last frame that still had it.
    if (m_unmapPending)
        return;
    m_unmapPending = true;
    connect(m_window, &QQuickWindow::frameSwapped, this, [this] {
        m_unmapPending = false;
        if (m_mapped && !wantPanel() && !m_toastsActive && m_progress <= 0)
            unmap();
    }, Qt::ConnectionType(Qt::QueuedConnection | Qt::SingleShotConnection));
    m_window->update();
}

void OverlayController::map()
{
    qCDebug(lcWindow) << "mapping on" << (m_store->output().isEmpty() ? u"the active screen"_s : m_store->output());
    chooseScreen();
    applyPlacement();
    m_mapped = true;
    m_mappedForPanel = wantPanel();
    m_awaitingFirstFrame = true;
    connect(m_window, &QQuickWindow::frameSwapped, this, &OverlayController::onFirstFrame,
            Qt::ConnectionType(Qt::QueuedConnection | Qt::SingleShotConnection));
    m_window->show();
}

void OverlayController::unmap()
{
    qCDebug(lcWindow) << "unmapping";
    m_mapped = false;
    m_awaitingFirstFrame = false;
    setReady(false); // stops any running slide in QML before progress is reset
    if (m_progress != 0) {
        m_progress = 0;
        emit progressChanged();
    }
    setPresented(false);
    m_blurApplied = false;
    m_blurRegion = QRegion();
    m_window->hide();
}

void OverlayController::onFirstFrame()
{
    if (!m_mapped || !m_awaitingFirstFrame)
        return;
    m_awaitingFirstFrame = false;
    qCDebug(lcWindow) << "first frame on screen" << (m_window->screen() ? m_window->screen()->name() : QString());
    applyInputRegion();
    setReady(true);
    sync();
}

void OverlayController::onWindowHidden()
{
    if (!m_mapped)
        return;
    // Not initiated by us: the compositor dismissed the surface (e.g. its output went away).
    qCInfo(lcWindow) << "overlay surface was dismissed; remapping if still needed";
    unmap();
    scheduleSync();
}

void OverlayController::setPresented(bool presented)
{
    if (presented == m_presented)
        return;
    m_presented = presented;
    applyInputRegion();
    emit presentedChanged();
}

void OverlayController::setReady(bool ready)
{
    if (ready == m_ready)
        return;
    m_ready = ready;
    emit readyChanged();
}

void OverlayController::setProgress(qreal progress)
{
    progress = std::clamp<qreal>(progress, 0, 1);
    if (progress == m_progress || (!m_ready && progress > 0))
        return;
    m_progress = progress;
    emit progressChanged();
    applyBlur();
    if (m_progress <= 0)
        scheduleSync();
}

void OverlayController::setToastsActive(bool active)
{
    if (active == m_toastsActive)
        return;
    m_toastsActive = active;
    emit toastsActiveChanged();
    scheduleSync();
}

void OverlayController::chooseScreen()
{
    const QString output = m_store->output();
    m_mappedOutput = output;
    QScreen *screen = screenNamed(output);
    if (!output.isEmpty() && !screen)
        qCInfo(lcWindow) << "output" << output << "not found; letting the compositor choose";
#ifdef STREAMPAD_HAVE_LAYERSHELL
    if (m_layerShell) {
        auto *surface = LayerShellQt::Window::get(m_window);
        if (screen) {
            m_window->setScreen(screen);
            surface->setScreen(screen);
        } else {
            surface->setWantsToBeOnActiveScreen(true);
        }
        return;
    }
#endif
    if (!screen)
        screen = QGuiApplication::screenAt(QCursor::pos());
    if (!screen)
        screen = QGuiApplication::primaryScreen();
    if (screen)
        m_window->setScreen(screen);
}

void OverlayController::applyPlacement()
{
    m_window->resize(m_geometry.windowSize());
#ifdef STREAMPAD_HAVE_LAYERSHELL
    if (m_layerShell) {
        auto *surface = LayerShellQt::Window::get(m_window);
        surface->setAnchors(anchorsFor(m_geometry));
        surface->setMargins(marginsFor(m_geometry));
        return;
    }
#endif
    if (const QScreen *screen = m_window->screen())
        m_window->setPosition(fallbackPosition(screen->geometry(), m_geometry));
}

void OverlayController::applyInputRegion()
{
    // Without layer-shell the window is always transparent for input (see attach()).
    if (!m_window || !m_mapped || !m_layerShell)
        return;
    // Only the panel shape takes input, and only while it is (being) presented; toasts never do.
    m_window->setFlag(Qt::WindowTransparentForInput, !m_presented);
    if (m_presented)
        m_window->setMask(m_geometry.inputRegion());
}

QPoint OverlayController::slideOffset() const
{
    return (m_geometry.slideVector() * (1 - m_progress)).toPoint();
}

void OverlayController::applyBlur()
{
#ifdef STREAMPAD_HAVE_KWINDOWSYSTEM
    if (!m_window || !m_ready || !m_layerShell)
        return;
    QRegion region;
    if (m_store->settings().blur && m_progress > 0)
        region = m_geometry.inputRegion().translated(slideOffset()) & QRegion(QRect(QPoint(), m_window->size()));
    // An empty region would mean "blur the whole window", so an empty shape disables blur instead.
    const bool enable = !region.isEmpty();
    if (enable == m_blurApplied && region == m_blurRegion)
        return;
    KWindowEffects::enableBlurBehind(m_window, enable, region);
    m_blurApplied = enable;
    m_blurRegion = region;
#endif
}
