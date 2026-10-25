#pragma once

#include <QFileSystemWatcher>
#include <QJsonObject>
#include <QObject>
#include <QString>
#include <QTimer>

// Feeds a DaemonState from a JSON file instead of the daemon and re-emits it whenever the file changes.
// For portable fixtures, icon refs may be "lucide:<name>" and asset paths may be relative to the file.
class MockSource : public QObject
{
    Q_OBJECT

public:
    explicit MockSource(const QString &path, QObject *parent = nullptr);

    // Loads the file now; returns false (and logs why) if it could not be read or parsed.
    bool load();

signals:
    void stateReceived(const QJsonObject &state);

private:
    QJsonObject resolveRefs(QJsonObject state) const;
    QString resolveAsset(const QString &ref) const;
    void rewatch();

    QString m_path;
    QString m_baseDir;
    QString m_shareDir;
    QFileSystemWatcher m_watcher;
    QTimer m_reloadTimer;
};
