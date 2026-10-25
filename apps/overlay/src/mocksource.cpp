#include "mocksource.h"

#include "paths.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonParseError>
#include <QLoggingCategory>

Q_LOGGING_CATEGORY(lcMock, "streampad.overlay.mock", QtInfoMsg)

namespace {

constexpr qsizetype kMaxMockBytes = 16 * 1024 * 1024;
constexpr QLatin1StringView kLucidePrefix("lucide:");

} // namespace

MockSource::MockSource(const QString &path, QObject *parent)
    : QObject(parent),
      m_path(QFileInfo(path).absoluteFilePath()),
      m_baseDir(QFileInfo(m_path).absolutePath()),
      m_shareDir(streampad::paths::shareDir())
{
    // Editors and the fixture tools replace the file atomically, which drops it from the watch list;
    // watching the directory as well catches the rename.
    m_reloadTimer.setSingleShot(true);
    m_reloadTimer.setInterval(60);
    connect(&m_reloadTimer, &QTimer::timeout, this, [this] {
        rewatch();
        load();
    });
    connect(&m_watcher, &QFileSystemWatcher::fileChanged, &m_reloadTimer, qOverload<>(&QTimer::start));
    connect(&m_watcher, &QFileSystemWatcher::directoryChanged, &m_reloadTimer, qOverload<>(&QTimer::start));
    rewatch();
}

void MockSource::rewatch()
{
    if (!m_watcher.directories().contains(m_baseDir))
        m_watcher.addPath(m_baseDir);
    if (QFileInfo::exists(m_path) && !m_watcher.files().contains(m_path))
        m_watcher.addPath(m_path);
}

bool MockSource::load()
{
    QFile file(m_path);
    if (!file.open(QIODevice::ReadOnly)) {
        qCWarning(lcMock) << "cannot read" << m_path << ":" << file.errorString();
        return false;
    }
    const QByteArray bytes = file.read(kMaxMockBytes);
    QJsonParseError error;
    const QJsonDocument doc = QJsonDocument::fromJson(bytes, &error);
    if (!doc.isObject()) {
        qCWarning(lcMock) << "invalid JSON in" << m_path << ":" << error.errorString();
        return false;
    }
    emit stateReceived(resolveRefs(doc.object()));
    return true;
}

QString MockSource::resolveAsset(const QString &ref) const
{
    if (ref.startsWith(kLucidePrefix)) {
        const QString name = ref.mid(kLucidePrefix.size());
        if (m_shareDir.isEmpty() || name.contains(u'/'))
            return {};
        const QString path = m_shareDir + QStringLiteral("/icons/lucide/") + name + QStringLiteral(".svg");
        return QFileInfo::exists(path) ? path : QString();
    }
    if (ref.isEmpty() || QDir::isAbsolutePath(ref))
        return ref;
    return QDir(m_baseDir).absoluteFilePath(ref);
}

QJsonObject MockSource::resolveRefs(QJsonObject state) const
{
    const auto resolveField = [this](QJsonObject &object, QStringView field) {
        const QJsonValue value = object.value(field);
        if (value.isString())
            object.insert(field, resolveAsset(value.toString()));
    };

    QJsonObject profile = state.value(u"profile").toObject();
    resolveField(profile, u"icon");
    QJsonObject keys = profile.value(u"keys").toObject();
    for (auto it = keys.begin(); it != keys.end(); ++it) {
        QJsonObject key = it.value().toObject();
        if (key.value(u"icon").toString().startsWith(kLucidePrefix))
            key.insert(u"iconTint", true);
        resolveField(key, u"icon");
        resolveField(key, u"cover");
        *it = key;
    }
    profile.insert(u"keys", keys);
    state.insert(u"profile", profile);

    QJsonArray profiles = state.value(u"profiles").toArray();
    for (auto it = profiles.begin(); it != profiles.end(); ++it) {
        QJsonObject summary = it->toObject();
        resolveField(summary, u"icon");
        *it = summary;
    }
    state.insert(u"profiles", profiles);
    return state;
}
