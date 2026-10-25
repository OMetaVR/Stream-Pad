#include "paths.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QStringList>

#include <unistd.h>

namespace streampad::paths {

namespace {

QString envPath(const char *name)
{
    return qEnvironmentVariable(name).trimmed();
}

QString homeDir()
{
    const QString home = envPath("HOME");
    return home.isEmpty() ? QStringLiteral("/tmp") : home;
}

QString xdgDir(const char *name, const char *fallbackUnderHome)
{
    const QString dir = envPath(name);
    return dir.isEmpty() ? homeDir() + u'/' + QLatin1StringView(fallbackUnderHome) : dir;
}

QString runtimeDir()
{
    const QString overridden = envPath("STREAMPAD_RUNTIME_DIR");
    if (!overridden.isEmpty())
        return overridden;
    QString base = envPath("XDG_RUNTIME_DIR");
    if (base.isEmpty()) {
        const QString tmp = envPath("TMPDIR");
        base = (tmp.isEmpty() ? QStringLiteral("/tmp") : tmp) + QStringLiteral("/streampad-") + QString::number(getuid());
    }
    return base + QStringLiteral("/streampad");
}

QString dataDir()
{
    const QString overridden = envPath("STREAMPAD_DATA_DIR");
    return overridden.isEmpty() ? xdgDir("XDG_DATA_HOME", ".local/share") + QStringLiteral("/streampad") : overridden;
}

} // namespace

QString socketPath()
{
    const QString overridden = envPath("STREAMPAD_SOCKET");
    return overridden.isEmpty() ? runtimeDir() + QStringLiteral("/daemon.sock") : overridden;
}

QString shareDir()
{
    QStringList candidates;
    if (const QString env = envPath("STREAMPAD_SHARE_DIR"); !env.isEmpty())
        candidates << env;
    const QString bin = QCoreApplication::applicationDirPath();
    candidates << bin + QStringLiteral("/../share/streampad")
               // dev layout: apps/overlay/build/streampad-overlay → <repo>/build/share/streampad
               << bin + QStringLiteral("/../../../build/share/streampad")
               << dataDir() + QStringLiteral("/share") << QStringLiteral("/usr/local/share/streampad")
               << QStringLiteral("/usr/share/streampad");
    for (const QString &candidate : std::as_const(candidates)) {
        if (QFileInfo(candidate + QStringLiteral("/icons")).isDir())
            return QDir(candidate).canonicalPath();
    }
    return {};
}

} // namespace streampad::paths
