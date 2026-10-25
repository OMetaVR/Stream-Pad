#include "panelbackground.h"

#include "panelgeometry.h"

#include <QPainter>
#include <QQuickWindow>

#include <algorithm>
#include <cmath>
#include <vector>

namespace {

struct ShadowLayer {
    QPointF offset;  // logical px (before scale)
    qreal sigma;     // logical px (before scale)
    qreal strength;  // multiplies the shadow colour's alpha
};

// A tight contact shadow plus a wide, slightly dropped ambient one.
constexpr ShadowLayer kShadowLayers[] = {
    {{0, 1}, 2.5, 0.45},
    {{0, 4}, 12, 1.0},
};

// Box blur along one line (stride `step`); out-of-range samples count as transparent.
void blurLine(const uchar *in, uchar *out, int n, qsizetype step, int radius)
{
    const int window = 2 * radius + 1;
    int sum = 0;
    for (int i = 0; i <= std::min(radius, n - 1); ++i)
        sum += in[i * step];
    for (int i = 0; i < n; ++i) {
        out[i * step] = uchar((sum + window / 2) / window);
        if (const int add = i + radius + 1; add < n)
            sum += in[add * step];
        if (const int sub = i - radius; sub >= 0)
            sum -= in[sub * step];
    }
}

// Three box passes per axis approximate a Gaussian with σ² = r² + r.
void blurAlpha(std::vector<uchar> &alpha, int width, int height, qreal sigma)
{
    const int radius = int(std::round(std::sqrt(sigma * sigma + 0.25) - 0.5));
    if (radius <= 0)
        return;
    std::vector<uchar> tmp(alpha.size());
    for (int pass = 0; pass < 3; ++pass) {
        for (int y = 0; y < height; ++y)
            blurLine(&alpha[size_t(y) * width], &tmp[size_t(y) * width], width, 1, radius);
        for (int x = 0; x < width; ++x)
            blurLine(&tmp[x], &alpha[x], height, width, radius);
    }
}

std::vector<uchar> rasterizeAlpha(const QPainterPath &path, const QRectF &area, qreal dpr, const QSize &pixels,
                                  const QPointF &offset, qreal opacity)
{
    QImage image(pixels, QImage::Format_ARGB32_Premultiplied);
    image.fill(Qt::transparent);
    {
        QPainter painter(&image);
        painter.setRenderHint(QPainter::Antialiasing);
        painter.scale(dpr, dpr);
        painter.translate(offset - area.topLeft());
        painter.fillPath(path, QColor::fromRgbF(0, 0, 0, float(opacity)));
    }
    std::vector<uchar> alpha(size_t(pixels.width()) * pixels.height());
    for (int y = 0; y < pixels.height(); ++y) {
        const auto *row = reinterpret_cast<const QRgb *>(image.constScanLine(y));
        for (int x = 0; x < pixels.width(); ++x)
            alpha[size_t(y) * pixels.width() + x] = uchar(qAlpha(row[x]));
    }
    return alpha;
}

} // namespace

PanelBackground::PanelBackground(QQuickItem *parent) : QQuickPaintedItem(parent)
{
    setAntialiasing(true);
}

void PanelBackground::setGeometry(PanelGeometry *geometry)
{
    if (geometry == m_geometry)
        return;
    if (m_geometry)
        disconnect(m_geometry, nullptr, this, nullptr);
    m_geometry = geometry;
    if (m_geometry)
        connect(m_geometry, &PanelGeometry::changed, this, &PanelBackground::invalidateShadow);
    invalidateShadow();
    emit geometryChanged();
}

void PanelBackground::setPanelColor(const QColor &color)
{
    if (color == m_panelColor)
        return;
    m_panelColor = color;
    update();
    emit panelColorChanged();
}

void PanelBackground::setBorderColor(const QColor &color)
{
    if (color == m_borderColor)
        return;
    m_borderColor = color;
    update();
    emit borderColorChanged();
}

void PanelBackground::setShadowColor(const QColor &color)
{
    if (color == m_shadowColor)
        return;
    m_shadowColor = color;
    invalidateShadow();
    emit shadowColorChanged();
}

void PanelBackground::invalidateShadow()
{
    m_shadow = QImage();
    update();
}

QImage PanelBackground::renderShadow(qreal dpr) const
{
    const QRectF area = m_geometry->backgroundRect();
    const QSize pixels = (area.size() * dpr).toSize();
    if (pixels.isEmpty() || m_shadowColor.alpha() == 0)
        return {};

    const qreal s = m_geometry->scale();
    std::vector<uchar> combined(size_t(pixels.width()) * pixels.height(), 0);
    for (const ShadowLayer &layer : kShadowLayers) {
        std::vector<uchar> alpha = rasterizeAlpha(m_geometry->fillPath(), area, dpr, pixels, layer.offset * s,
                                                  std::min<qreal>(1.0, m_shadowColor.alphaF() * layer.strength));
        blurAlpha(alpha, pixels.width(), pixels.height(), layer.sigma * s * dpr);
        for (size_t i = 0; i < combined.size(); ++i)
            combined[i] = uchar(255 - (255 - combined[i]) * (255 - alpha[i]) / 255);
    }

    QImage shadow(pixels, QImage::Format_ARGB32_Premultiplied);
    const int r = m_shadowColor.red(), g = m_shadowColor.green(), b = m_shadowColor.blue();
    for (int y = 0; y < pixels.height(); ++y) {
        auto *row = reinterpret_cast<QRgb *>(shadow.scanLine(y));
        for (int x = 0; x < pixels.width(); ++x) {
            const int a = combined[size_t(y) * pixels.width() + x];
            row[x] = qRgba(r * a / 255, g * a / 255, b * a / 255, a);
        }
    }

    // Knock the shadow out under the panel itself.
    QPainter painter(&shadow);
    painter.setRenderHint(QPainter::Antialiasing);
    painter.setCompositionMode(QPainter::CompositionMode_Clear);
    painter.scale(dpr, dpr);
    painter.translate(-area.topLeft());
    painter.fillPath(m_geometry->fillPath(), Qt::black);
    painter.end();

    shadow.setDevicePixelRatio(dpr);
    return shadow;
}

void PanelBackground::paint(QPainter *painter)
{
    if (!m_geometry)
        return;
    const qreal dpr = window() ? window()->effectiveDevicePixelRatio() : 1.0;
    if (m_shadow.isNull() || dpr != m_shadowDpr) {
        m_shadow = renderShadow(dpr);
        m_shadowDpr = dpr;
    }

    const QRectF area = m_geometry->backgroundRect();
    painter->setRenderHint(QPainter::Antialiasing);
    if (!m_shadow.isNull())
        painter->drawImage(QPointF(0, 0), m_shadow);

    painter->translate(-area.topLeft());
    const QPainterPath &fill = m_geometry->fillPath();
    painter->fillPath(fill, m_panelColor);

    // A 2 px stroke clipped to the shape leaves a crisp 1 px hairline just inside the outline.
    painter->setClipPath(fill);
    QPen pen(m_borderColor, 2.0);
    pen.setJoinStyle(Qt::RoundJoin);
    painter->strokePath(m_geometry->borderPath(), pen);
}
