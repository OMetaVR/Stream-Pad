#include "overlaysettings.h"

#include <QJsonValue>

#include <algorithm>
#include <cmath>
#include <initializer_list>

namespace {

QString pickEnum(const QJsonValue &value, std::initializer_list<const char *> allowed, const QString &fallback)
{
    const QString text = value.toString();
    for (const char *option : allowed) {
        if (text == QLatin1StringView(option))
            return text;
    }
    return fallback;
}

qreal clampedReal(const QJsonValue &value, qreal fallback, qreal lo, qreal hi)
{
    const double number = value.toDouble(fallback);
    return std::isfinite(number) ? std::clamp<qreal>(number, lo, hi) : fallback;
}

int clampedInt(const QJsonValue &value, int fallback, int lo, int hi)
{
    if (!value.isDouble())
        return fallback;
    return static_cast<int>(std::clamp<double>(std::round(value.toDouble()), lo, hi));
}

bool boolOr(const QJsonValue &value, bool fallback)
{
    return value.isBool() ? value.toBool() : fallback;
}

} // namespace

QColor parseColor(const QJsonValue &value, const QColor &fallback)
{
    QString text = value.toString().trimmed();
    if (text.isEmpty())
        return fallback;
    // JSON producers (editor, daemon) use CSS order #rrggbbaa; Qt expects #aarrggbb.
    if (text.size() == 9 && text.startsWith(u'#'))
        text = u'#' + text.mid(7, 2) + text.mid(1, 6);
    const QColor color = QColor::fromString(text);
    return color.isValid() ? color : fallback;
}

OverlaySettings OverlaySettings::fromJson(const QJsonObject &json)
{
    const OverlaySettings d;
    OverlaySettings s;
    s.edge = pickEnum(json.value(u"edge"), {"bottom", "top", "left", "right"}, d.edge);
    s.align = pickEnum(json.value(u"align"), {"start", "center", "end"}, d.align);
    s.offset = clampedInt(json.value(u"offset"), d.offset, 0, 10000);
    s.scale = clampedReal(json.value(u"scale"), d.scale, 0.5, 2.0);
    s.theme = pickEnum(json.value(u"theme"), {"dark", "light"}, d.theme);
    s.accent = parseColor(json.value(u"accent"), d.accent);
    s.opacity = clampedReal(json.value(u"opacity"), d.opacity, 0.3, 1.0);
    s.blur = boolOr(json.value(u"blur"), d.blur);
    s.showHeader = boolOr(json.value(u"showHeader"), d.showHeader);
    s.showLegends = boolOr(json.value(u"showLegends"), d.showLegends);
    s.legendStyle = pickEnum(json.value(u"legendStyle"), {"full", "number"}, d.legendStyle);
    s.showLabels = boolOr(json.value(u"showLabels"), d.showLabels);
    s.animationMs = clampedInt(json.value(u"animationMs"), d.animationMs, 0, 2000);
    return s;
}

bool OverlaySettings::sameLayout(const OverlaySettings &other) const
{
    return edge == other.edge && align == other.align && offset == other.offset && scale == other.scale
        && showHeader == other.showHeader;
}
