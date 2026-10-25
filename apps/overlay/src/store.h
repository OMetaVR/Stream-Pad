#pragma once

#include "overlaysettings.h"

#include <QColor>
#include <QJsonObject>
#include <QObject>
#include <QString>
#include <QVariantList>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>

class QQmlEngine;
class QJSEngine;

// Render-ready view of the daemon state (docs/SPEC.md §4) for QML, plus the presentational events.
// Input is normalized here so QML never sees missing fields: every profile carries all 17 keys.
class OverlayStore : public QObject
{
    Q_OBJECT
    QML_NAMED_ELEMENT(Store)
    QML_SINGLETON
    Q_PROPERTY(bool connected READ connected NOTIFY connectedChanged)
    Q_PROPERTY(bool visible READ visible NOTIFY visibleChanged)
    Q_PROPERTY(OverlaySettings settings READ settings NOTIFY settingsChanged)
    Q_PROPERTY(QColor accent READ accent NOTIFY accentChanged)
    Q_PROPERTY(int profileIndex READ profileIndex NOTIFY profilesChanged)
    Q_PROPERTY(int profileCount READ profileCount NOTIFY profilesChanged)
    // Current profile: {id, name, iconId, keys: {KeyId: key}}; see normalizeKey() for the key shape.
    Q_PROPERTY(QVariantMap profile READ profile NOTIFY profileChanged)
    // Direction of the most recent profile switch (+1 next, -1 previous); valid in onProfileChanged.
    Q_PROPERTY(int profileDirection READ profileDirection NOTIFY profileChanged)
    // [{id, col, row, colSpan, rowSpan}] for the 17 pad keys.
    Q_PROPERTY(QVariantList keyLayout READ keyLayout CONSTANT)

public:
    // Deliberately not default-constructible: QML must use create() and share this instance.
    explicit OverlayStore(QObject *parent);
    ~OverlayStore() override;

    static OverlayStore *create(QQmlEngine *, QJSEngine *engine);

    void applyState(const QJsonObject &state);
    void applyEvent(const QString &event, const QJsonObject &data);
    void setConnected(bool connected);

    bool connected() const { return m_connected; }
    bool visible() const { return m_connected && m_stateVisible; }
    // Connector name the daemon wants the overlay on; empty = let the compositor decide.
    QString output() const { return m_output; }
    OverlaySettings settings() const { return m_settings; }
    QColor accent() const { return m_accent; }
    int profileIndex() const { return m_profileIndex; }
    int profileCount() const { return m_profileCount; }
    QVariantMap profile() const { return m_profile; }
    int profileDirection() const { return m_profileDirection; }
    QVariantList keyLayout() const;

signals:
    void connectedChanged();
    void visibleChanged();
    void outputChanged();
    void settingsChanged();
    void accentChanged();
    void profilesChanged();
    void profileChanged();
    void keyEvent(const QString &key, bool pressed, const QString &kind);
    // {id, text, kind: info|success|error, durationMs, iconId}
    void toastReceived(const QVariantMap &toast);

private:
    void setStateVisible(bool visible);

    bool m_connected = false;
    bool m_stateVisible = false;
    QString m_output;
    OverlaySettings m_settings;
    QColor m_accent = OverlaySettings().accent;
    int m_profileIndex = 0;
    int m_profileCount = 0;
    QVariantMap m_profile;
    int m_profileDirection = 1;
    int m_pendingDirection = 0; // from a `profile` event, consumed by the next state
};
