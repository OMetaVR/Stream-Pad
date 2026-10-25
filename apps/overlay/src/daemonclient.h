#pragma once

#include <QByteArray>
#include <QJsonArray>
#include <QJsonObject>
#include <QJsonValue>
#include <QLocalSocket>
#include <QObject>
#include <QTimer>

// Newline-delimited JSON client for the daemon socket (docs/SPEC.md §4). Subscribes as the overlay,
// reports the screens, and reconnects with exponential backoff forever.
class DaemonClient : public QObject
{
    Q_OBJECT

public:
    explicit DaemonClient(QString socketPath, QObject *parent = nullptr);

    void start();
    // Remembered and re-sent after every reconnect.
    void reportOutputs(const QJsonArray &outputs);

signals:
    void connectedChanged(bool connected);
    void stateReceived(const QJsonObject &state);
    void eventReceived(const QString &event, const QJsonObject &data);

private:
    void connectNow();
    void onConnected();
    void onDisconnected();
    void onReadyRead();
    void scheduleReconnect();
    void handleLine(const QByteArray &line);
    void send(const QString &method, const QJsonValue &params);

    QString m_socketPath;
    QLocalSocket m_socket;
    QTimer m_retryTimer;
    QByteArray m_buffer;
    QJsonArray m_outputs;
    quint64 m_nextId = 1;
    quint64 m_helloId = 0;
    int m_backoffMs = 0;
    bool m_connected = false;
    bool m_loggedFailure = false;
};
