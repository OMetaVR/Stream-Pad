#pragma once

#include <QString>
#include <QStringList>

class QCoreApplication;

namespace streampad {

struct Options {
    QString socketPath;
    // Dev/test: render from a DaemonState JSON file instead of the daemon.
    QString mockFile;
    QString screenshotPath;
    int screenshotAfterMs = -1; // < 0: capture once the panel is at rest
    QStringList mockKeys;       // KeyIds held down once the panel is shown
    QStringList mockToasts;     // "[info|success|error:]text"
    // Frame-sequence capture on a fixed-step clock (needs no daemon and no --mock).
    QString captureScript;
    QString captureDir;
};

// Parses the command line; prints usage and exits on invalid input.
Options parseOptions(const QCoreApplication &app);

} // namespace streampad
