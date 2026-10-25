#include "capture.h"
#include "controller.h"
#include "daemonclient.h"
#include "iconprovider.h"
#include "mocksource.h"
#include "options.h"
#include "outputs.h"
#include "platform.h"
#include "screenshot.h"
#include "steppedclock.h"
#include "store.h"

#include <QFont>
#include <QFontDatabase>
#include <QGuiApplication>
#include <QJsonObject>
#include <QLoggingCategory>
#include <QQmlApplicationEngine>
#include <QQuickWindow>
#include <QTimer>

#include <memory>

using namespace Qt::StringLiterals;

Q_LOGGING_CATEGORY(lcMain, "streampad.overlay", QtInfoMsg)

namespace {

void useBundledFont()
{
    const int id = QFontDatabase::addApplicationFont(u":/qt/qml/StreamPad/Overlay/fonts/InterVariable.ttf"_s);
    const QStringList families = QFontDatabase::applicationFontFamilies(id);
    if (families.isEmpty()) {
        qCWarning(lcMain) << "bundled Inter font failed to load; using the system sans-serif";
        return;
    }
    QFont font(families.first());
    font.setStyleStrategy(QFont::StyleStrategy(QFont::PreferAntialias | QFont::NoSubpixelAntialias));
    QGuiApplication::setFont(font);
}

// "[info|success|error:]text" → toast event payload.
QJsonObject mockToast(const QString &spec, int id)
{
    QString kind = u"info"_s;
    QString text = spec;
    const qsizetype colon = spec.indexOf(u':');
    if (const QString prefix = spec.left(colon); colon > 0 && (prefix == u"info" || prefix == u"success" || prefix == u"error")) {
        kind = prefix;
        text = spec.mid(colon + 1);
    }
    return {{u"id"_s, id}, {u"text"_s, text}, {u"kind"_s, kind}, {u"durationMs"_s, 5000}};
}

// --mock-toast / --mock-key are delivered like daemon events: toasts as the panel starts to show,
// key presses once it is at rest.
void scheduleMockInput(OverlayController &controller, OverlayStore &store, const streampad::Options &options)
{
    if (!options.mockToasts.isEmpty()) {
        QObject::connect(
            &controller, &OverlayController::presentedChanged, &store,
            [&store, toasts = options.mockToasts] {
                int id = 1;
                for (const QString &toast : toasts)
                    store.applyEvent(u"toast"_s, mockToast(toast, id++));
            },
            Qt::SingleShotConnection);
    }
    if (!options.mockKeys.isEmpty()) {
        auto delivered = std::make_shared<bool>(false);
        QObject::connect(&controller, &OverlayController::progressChanged, &store,
                         [&controller, &store, delivered, keys = options.mockKeys] {
                             if (*delivered || controller.progress() < 1)
                                 return;
                             *delivered = true;
                             for (const QString &key : keys)
                                 store.applyEvent(u"key"_s, {{u"key"_s, key}, {u"pressed"_s, true}, {u"kind"_s, u"macro"_s}});
                         });
    }
}

} // namespace

int main(int argc, char *argv[])
{
    streampad::platform::selectQpaPlatform();
    // Font sizes are specified in points derived from px (Theme.pt), so pin the DPI they map with.
    QCoreApplication::setAttribute(Qt::AA_Use96Dpi);
    QGuiApplication app(argc, argv);
    QGuiApplication::setApplicationName(u"streampad-overlay"_s);
    QGuiApplication::setApplicationVersion(QStringLiteral(STREAMPAD_OVERLAY_VERSION));
    // The window is hidden (and may be closed by the compositor) whenever nothing is shown.
    QGuiApplication::setQuitOnLastWindowClosed(false);
    const streampad::Options options = streampad::parseOptions(app);

    if (QGuiApplication::platformName() == u"offscreen") {
        // The offscreen platform defaults to the software renderer, which lacks the shader effects.
        if (qEnvironmentVariableIsEmpty("QT_QUICK_BACKEND") && qEnvironmentVariableIsEmpty("QSG_RHI_BACKEND")) {
            QQuickWindow::setSceneGraphBackend(u"rhi"_s);
            QQuickWindow::setGraphicsApi(QSGRendererInterface::OpenGL);
        }
        // Without vsync the threaded loop advances animations faster than real time, which would
        // make --after-ms captures meaningless.
        if (qEnvironmentVariableIsEmpty("QSG_RENDER_LOOP"))
            qputenv("QSG_RENDER_LOOP", "basic");
    }
    QQuickWindow::setDefaultAlphaBuffer(true);
    // Hinted native glyphs are the crispest at 10–14 px; the default distance-field path falls back to
    // subpixel-antialiased glyphs for the variable font, which fringe on a translucent overlay.
    QQuickWindow::setTextRenderType(QQuickWindow::NativeTextRendering);
    useBundledFont();

    // Capture mode: animations and QML timers must only ever see the virtual clock, so it goes in
    // before the engine creates any of them.
    std::unique_ptr<SteppedClock> clock;
    streampad::CaptureScript captureScript;
    if (!options.captureScript.isEmpty()) {
        QString error;
        if (!captureScript.load(options.captureScript, &error)) {
            qCCritical(lcMain) << error;
            return 2;
        }
        clock = std::make_unique<SteppedClock>();
    }

    OverlayStore store(nullptr);
    OverlayController controller(&store);

    QQmlApplicationEngine engine;
    engine.addImageProvider(u"icon"_s, new IconProvider);
    engine.loadFromModule("StreamPad.Overlay", "Main");
    auto *window = engine.rootObjects().isEmpty() ? nullptr : qobject_cast<QQuickWindow *>(engine.rootObjects().constFirst());
    if (!window) {
        qCCritical(lcMain) << "failed to load the overlay UI";
        return 1;
    }
    controller.attach(window);

    std::unique_ptr<MockSource> mock;
    std::unique_ptr<ScreenshotTask> screenshot;
    std::unique_ptr<CaptureTask> capture;
    std::unique_ptr<DaemonClient> client;
    std::unique_ptr<OutputWatcher> outputs;

    if (clock) {
        capture = std::make_unique<CaptureTask>(window, &controller, &store, clock.get(), captureScript,
                                                options.captureDir);
        QTimer::singleShot(0, capture.get(), &CaptureTask::run);
    } else if (!options.mockFile.isEmpty()) {
        store.setConnected(true);
        mock = std::make_unique<MockSource>(options.mockFile);
        QObject::connect(mock.get(), &MockSource::stateReceived, &store, &OverlayStore::applyState);
        if (!mock->load() && !options.screenshotPath.isEmpty())
            return 1;
        scheduleMockInput(controller, store, options);
        if (!options.screenshotPath.isEmpty()) {
            screenshot = std::make_unique<ScreenshotTask>(window, &controller, options.screenshotPath, options.screenshotAfterMs);
            controller.setForceVisible(true);
        }
    } else {
        client = std::make_unique<DaemonClient>(options.socketPath);
        outputs = std::make_unique<OutputWatcher>();
        QObject::connect(client.get(), &DaemonClient::connectedChanged, &store, &OverlayStore::setConnected);
        QObject::connect(client.get(), &DaemonClient::stateReceived, &store, &OverlayStore::applyState);
        QObject::connect(client.get(), &DaemonClient::eventReceived, &store, &OverlayStore::applyEvent);
        QObject::connect(outputs.get(), &OutputWatcher::changed, client.get(), &DaemonClient::reportOutputs);
        client->reportOutputs(OutputWatcher::describe());
        client->start();
    }

    return QGuiApplication::exec();
}
