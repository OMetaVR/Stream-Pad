#include "options.h"

#include "keytable.h"
#include "paths.h"

#include <QCommandLineParser>
#include <QCoreApplication>

#include <cstdio>
#include <cstdlib>

namespace streampad {

namespace {

[[noreturn]] void fail(const QString &message)
{
    std::fprintf(stderr, "streampad-overlay: %s\n", qPrintable(message));
    std::exit(2);
}

} // namespace

Options parseOptions(const QCoreApplication &app)
{
    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral("StreamPad on-screen numpad display."));
    parser.addHelpOption();
    parser.addVersionOption();

    const QCommandLineOption socket(QStringLiteral("socket"), QStringLiteral("Daemon socket path."), QStringLiteral("path"));
    const QCommandLineOption mock(QStringLiteral("mock"),
                                  QStringLiteral("Render from a DaemonState JSON file (watched for changes) instead of the daemon."),
                                  QStringLiteral("state.json"));
    const QCommandLineOption screenshot(QStringLiteral("screenshot"),
                                        QStringLiteral("Save the window content as PNG and exit (requires --mock)."),
                                        QStringLiteral("out.png"));
    const QCommandLineOption afterMs(QStringLiteral("after-ms"),
                                     QStringLiteral("Capture N ms after the show animation starts (default: once at rest)."),
                                     QStringLiteral("N"));
    const QCommandLineOption mockKey(QStringLiteral("mock-key"),
                                     QStringLiteral("Hold a key down once shown (repeatable), e.g. KP4."), QStringLiteral("KeyId"));
    const QCommandLineOption mockToast(QStringLiteral("mock-toast"),
                                       QStringLiteral("Show a toast (repeatable); prefix with info:, success: or error:."),
                                       QStringLiteral("text"));
    parser.addOptions({socket, mock, screenshot, afterMs, mockKey, mockToast});
    parser.process(app);

    Options options;
    options.socketPath = parser.isSet(socket) ? parser.value(socket) : paths::socketPath();
    options.mockFile = parser.value(mock);
    options.screenshotPath = parser.value(screenshot);
    options.mockKeys = parser.values(mockKey);
    options.mockToasts = parser.values(mockToast);

    if (parser.isSet(afterMs)) {
        bool ok = false;
        options.screenshotAfterMs = parser.value(afterMs).toInt(&ok);
        if (!ok || options.screenshotAfterMs < 0)
            fail(QStringLiteral("--after-ms expects a non-negative integer"));
        if (options.screenshotPath.isEmpty())
            fail(QStringLiteral("--after-ms requires --screenshot"));
    }
    const bool mockOnly = !options.screenshotPath.isEmpty() || !options.mockKeys.isEmpty() || !options.mockToasts.isEmpty();
    if (mockOnly && options.mockFile.isEmpty())
        fail(QStringLiteral("--screenshot, --mock-key and --mock-toast require --mock"));
    for (const QString &key : std::as_const(options.mockKeys)) {
        if (!findPadKey(key))
            fail(QStringLiteral("unknown key '%1' (expected a KeyId such as KP4 or KPEnter)").arg(key));
    }
    return options;
}

} // namespace streampad
