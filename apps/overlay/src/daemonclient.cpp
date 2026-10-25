#include "daemonclient.h"

#include <QJsonDocument>
#include <QJsonParseError>
#include <QLoggingCategory>

#include <algorithm>

using namespace Qt::StringLiterals;

Q_LOGGING_CATEGORY(lcClient, "streampad.overlay.client", QtInfoMsg)

namespace {

constexpr int kInitialBackoffMs = 200;
constexpr int kMaxBackoffMs = 2000;
// A single message larger than this means the stream is corrupt; drop the connection.
constexpr qsizetype kMaxLineBytes = 32 * 1024 * 1024;

} // namespace

DaemonClient::DaemonClient(QString socketPath, QObject *parent)
    : QObject(parent), m_socketPath(std::move(socketPath))
{
    m_retryTimer.setSingleShot(true);
    connect(&m_retryTimer, &QTimer::timeout, this, &DaemonClient::connectNow);
    connect(&m_socket, &QLocalSocket::connected, this, &DaemonClient::onConnected);
    connect(&m_socket, &QLocalSocket::disconnected, this, &DaemonClient::onDisconnected);
    connect(&m_socket, &QLocalSocket::readyRead, this, &DaemonClient::onReadyRead);
    connect(&m_socket, &QLocalSocket::errorOccurred, this, [this](QLocalSocket::LocalSocketError error) {
        if (m_connected)
            return; // a live connection reports its end through disconnected()
        if (!m_loggedFailure) {
            qCInfo(lcClient) << "daemon not reachable at" << m_socketPath << "(" << error << ") - retrying";
            m_loggedFailure = true;
        }
        m_socket.abort();
        scheduleReconnect();
    });
}

void DaemonClient::start()
{
    connectNow();
}

void DaemonClient::reportOutputs(const QJsonArray &outputs)
{
    m_outputs = outputs;
    if (m_connected)
        send(QStringLiteral("reportOutputs"), m_outputs);
}

void DaemonClient::connectNow()
{
    if (m_socket.state() != QLocalSocket::UnconnectedState)
        return;
    m_socket.connectToServer(m_socketPath);
}

void DaemonClient::scheduleReconnect()
{
    m_backoffMs = m_backoffMs == 0 ? kInitialBackoffMs : std::min(m_backoffMs * 2, kMaxBackoffMs);
    m_retryTimer.start(m_backoffMs);
}

void DaemonClient::onConnected()
{
    qCInfo(lcClient) << "connected to" << m_socketPath;
    m_connected = true;
    m_loggedFailure = false;
    m_backoffMs = 0;
    m_buffer.clear();
    m_helloId = m_nextId;
    send(QStringLiteral("hello"), QJsonObject{{u"client"_s, u"overlay"_s}, {u"subscribe"_s, true}});
    send(QStringLiteral("reportOutputs"), m_outputs);
    emit connectedChanged(true);
}

void DaemonClient::onDisconnected()
{
    if (!m_connected)
        return;
    qCInfo(lcClient) << "daemon connection closed; reconnecting";
    m_connected = false;
    m_buffer.clear();
    emit connectedChanged(false);
    scheduleReconnect();
}

void DaemonClient::onReadyRead()
{
    m_buffer.append(m_socket.readAll());
    qsizetype start = 0;
    for (qsizetype newline = m_buffer.indexOf('\n'); newline >= 0; newline = m_buffer.indexOf('\n', start)) {
        const QByteArray line = m_buffer.mid(start, newline - start).trimmed();
        start = newline + 1;
        if (!line.isEmpty())
            handleLine(line);
    }
    m_buffer.remove(0, start);
    if (m_buffer.size() > kMaxLineBytes) {
        qCWarning(lcClient) << "oversized message from daemon; reconnecting";
        m_socket.abort();
    }
}

void DaemonClient::handleLine(const QByteArray &line)
{
    QJsonParseError error;
    const QJsonDocument doc = QJsonDocument::fromJson(line, &error);
    if (!doc.isObject()) {
        qCWarning(lcClient) << "ignoring malformed message:" << error.errorString();
        return;
    }
    const QJsonObject message = doc.object();

    if (const QJsonValue event = message.value(u"event"); event.isString()) {
        const QJsonObject data = message.value(u"data").toObject();
        if (event.toString() == u"state")
            emit stateReceived(data);
        else
            emit eventReceived(event.toString(), data);
        return;
    }

    const QJsonValue id = message.value(u"id");
    if (const QJsonValue error = message.value(u"error"); error.isObject()) {
        qCWarning(lcClient) << "request" << id.toInteger() << "failed:" << error[u"message"].toString();
        return;
    }
    if (id.toInteger() == qint64(m_helloId)) {
        const QJsonObject state = message.value(u"result")[u"state"].toObject();
        if (!state.isEmpty())
            emit stateReceived(state);
    }
}

void DaemonClient::send(const QString &method, const QJsonValue &params)
{
    const QJsonObject request{{u"id"_s, qint64(m_nextId++)}, {u"method"_s, method}, {u"params"_s, params}};
    m_socket.write(QJsonDocument(request).toJson(QJsonDocument::Compact) + '\n');
}
