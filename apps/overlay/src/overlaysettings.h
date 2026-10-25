#pragma once

#include <QColor>
#include <QJsonObject>
#include <QObject>
#include <QString>
#include <QtQml/qqmlregistration.h>

// `settings.overlay` (docs/SPEC.md §3.2) as the overlay renders it. Parsing is tolerant: missing or
// invalid fields take the documented defaults and numbers are clamped to the ranges the daemon enforces.
struct OverlaySettings {
    Q_GADGET
    QML_VALUE_TYPE(overlaySettings)
    QML_STRUCTURED_VALUE
    Q_PROPERTY(QString edge MEMBER edge)
    Q_PROPERTY(QString align MEMBER align)
    Q_PROPERTY(int offset MEMBER offset)
    Q_PROPERTY(qreal scale MEMBER scale)
    Q_PROPERTY(QString theme MEMBER theme)
    Q_PROPERTY(QColor accent MEMBER accent)
    Q_PROPERTY(qreal opacity MEMBER opacity)
    Q_PROPERTY(bool blur MEMBER blur)
    Q_PROPERTY(bool showHeader MEMBER showHeader)
    Q_PROPERTY(bool showLegends MEMBER showLegends)
    Q_PROPERTY(QString legendStyle MEMBER legendStyle)
    Q_PROPERTY(bool showLabels MEMBER showLabels)
    Q_PROPERTY(int animationMs MEMBER animationMs)

public:
    QString edge = QStringLiteral("bottom");    // bottom | top | left | right
    QString align = QStringLiteral("end");      // start | center | end
    int offset = 0;
    qreal scale = 1.0;
    QString theme = QStringLiteral("dark");     // dark | light
    QColor accent = QColor(0x8b, 0x7c, 0xf6);
    qreal opacity = 0.94;
    bool blur = true;
    bool showHeader = true;
    bool showLegends = true;
    QString legendStyle = QStringLiteral("full"); // full | number
    bool showLabels = true;
    int animationMs = 320;

    static OverlaySettings fromJson(const QJsonObject &json);

    // True when the two differ in anything that changes the window/panel geometry.
    bool sameLayout(const OverlaySettings &other) const;

    bool operator==(const OverlaySettings &) const = default;
};

// "#rrggbb" / "#rgb" / "#aarrggbb" (and anything else QColor accepts) or the fallback.
QColor parseColor(const QJsonValue &value, const QColor &fallback);
