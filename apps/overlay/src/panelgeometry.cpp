#include "panelgeometry.h"

#include <QPolygon>

#include <algorithm>
#include <cmath>

namespace {

// Control-point distance (relative to the radius) of the cubic Bézier closest to a quarter circle.
constexpr qreal kQuarterArc = 0.5522847498;

// Emits canonical (u, v) points into a window-space QPainterPath.
template<typename Map>
class OutlineBuilder
{
public:
    OutlineBuilder(QPainterPath &path, Map map) : m_path(path), m_map(map) {}

    void moveTo(QPointF p)
    {
        m_last = p;
        m_path.moveTo(m_map(p));
    }
    void lineTo(QPointF p)
    {
        m_last = p;
        m_path.lineTo(m_map(p));
    }
    // Quarter-circle arc from the current point to `to`; `corner` is where the two tangents meet.
    // The same construction yields convex corners and concave fillets.
    void arcTo(QPointF corner, QPointF to)
    {
        const QPointF c1 = m_last + (corner - m_last) * kQuarterArc;
        const QPointF c2 = to + (corner - to) * kQuarterArc;
        m_path.cubicTo(m_map(c1), m_map(c2), m_map(to));
        m_last = to;
    }

private:
    QPainterPath &m_path;
    Map m_map;
    QPointF m_last;
};

} // namespace

PanelGeometry::PanelGeometry(QObject *parent) : QObject(parent)
{
    recompute();
}

PanelGeometry::Edge PanelGeometry::parseEdge(const QString &edge)
{
    if (edge == u"top")
        return Edge::Top;
    if (edge == u"left")
        return Edge::Left;
    if (edge == u"right")
        return Edge::Right;
    return Edge::Bottom;
}

PanelGeometry::Align PanelGeometry::parseAlign(const QString &align)
{
    if (align == u"start")
        return Align::Start;
    if (align == u"center")
        return Align::Center;
    return Align::End;
}

void PanelGeometry::setLayout(const OverlaySettings &settings)
{
    const Edge edge = parseEdge(settings.edge);
    const Align align = parseAlign(settings.align);
    const int offset = std::max(0, settings.offset);
    if (m_initialized && edge == m_edge && align == m_align && offset == m_offset && settings.scale == m_scale
        && settings.showHeader == m_showHeader)
        return;
    m_edge = edge;
    m_align = align;
    m_offset = offset;
    m_scale = settings.scale;
    m_showHeader = settings.showHeader;
    m_initialized = true;
    recompute();
    emit changed();
}

QPointF PanelGeometry::toWindow(qreal u, qreal v) const
{
    switch (m_edge) {
    case Edge::Bottom:
        return {u, m_extentV - v};
    case Edge::Top:
        return {u, v};
    case Edge::Left:
        return {v, u};
    case Edge::Right:
        return {m_extentV - v, u};
    }
    return {u, v};
}

QRectF PanelGeometry::rectToWindow(qreal u0, qreal v0, qreal u1, qreal v1) const
{
    return QRectF(toWindow(u0, v0), toWindow(u1, v1)).normalized();
}

QPointF PanelGeometry::vectorToWindow(qreal du, qreal dv) const
{
    return toWindow(du, dv) - toWindow(0, 0);
}

QPainterPath PanelGeometry::buildOutline(bool closed) const
{
    QPainterPath path;
    OutlineBuilder b(path, [this](QPointF p) { return toWindow(p.x(), p.y()); });
    const qreal u0 = m_u0, u1 = m_u1, pv = m_pv, r = m_outerRadius, f = m_fillet;
    const bool flareStart = m_corner && m_align == Align::Start;
    const bool flareEnd = m_corner && m_align == Align::End;

    // Start side, from the screen edge to the far side. Flush segments are only part of the fill.
    if (flareStart) {
        if (closed) {
            b.moveTo({u0, 0});
            b.lineTo({u0, pv + f});
        } else {
            b.moveTo({u0, pv + f});
        }
        b.arcTo({u0, pv}, {u0 + f, pv});
    } else {
        b.moveTo({u0 - m_filletStart, 0});
        if (m_filletStart > 0)
            b.arcTo({u0, 0}, {u0, m_filletStart});
        b.lineTo({u0, pv - r});
        b.arcTo({u0, pv}, {u0 + r, pv});
    }

    // End side, from the far side back to the screen edge.
    if (flareEnd) {
        b.lineTo({u1 - f, pv});
        b.arcTo({u1, pv}, {u1, pv + f});
        if (closed)
            b.lineTo({u1, 0});
    } else {
        b.lineTo({u1 - r, pv});
        b.arcTo({u1, pv}, {u1, pv - r});
        b.lineTo({u1, m_filletEnd});
        if (m_filletEnd > 0)
            b.arcTo({u1, 0}, {u1 + m_filletEnd, 0});
    }

    if (closed)
        path.closeSubpath();
    return path;
}

void PanelGeometry::recompute()
{
    const qreal s = m_scale;
    // Grid metrics are rounded to whole logical pixels so keycaps stay crisp at any scale.
    const auto px = [s](qreal v) { return std::round(v * s); };
    m_unit = px(58);
    m_gap = px(8);
    m_pad = px(14);
    m_header = m_showHeader ? px(40) : 0;
    m_keyRadius = 11 * s;
    m_outerRadius = 22 * s;
    m_fillet = 20 * s;
    m_shadow = px(32);
    m_toastHeight = px(34);
    m_toastGap = px(8);
    m_toastMaxWidth = px(300);
    m_toastMargin = px(12);
    m_laneGap = px(30);

    const qreal gridW = 4 * m_unit + 3 * m_gap;
    const qreal gridH = 5 * m_unit + 4 * m_gap;
    const qreal panelW = gridW + 2 * m_pad;
    const qreal panelH = gridH + 2 * m_pad + m_header;
    const qreal pu = horizontal() ? panelW : panelH;
    m_pv = horizontal() ? panelH : panelW;

    // Toasts stack away from the edge on horizontal edges and along it on vertical ones.
    const qreal stack = 3 * m_toastHeight + 2 * m_toastGap;
    const qreal toastShadow = px(16);
    const qreal laneU = m_laneGap + (horizontal() ? m_toastMaxWidth : stack) + toastShadow;
    const qreal laneV = m_toastMargin + (horizontal() ? stack : m_toastMaxWidth) + toastShadow;

    m_corner = m_align != Align::Center && m_offset == 0;
    // The aligned side only needs room for the fillet + shadow up to the requested offset; the window
    // margin covers the rest. The opposite side hosts the toast lane.
    const qreal aligned = m_corner ? 0 : std::min<qreal>(m_offset, m_shadow);
    const qreal open = std::max(m_shadow, laneU);
    const qreal startSide = m_align == Align::Start ? aligned : open;
    const qreal endSide = m_align == Align::End ? aligned : open;
    m_alignedMargin = m_align == Align::Center ? 0 : m_offset - static_cast<int>(aligned);

    m_u0 = startSide;
    m_u1 = startSide + pu;
    m_extentU = std::ceil(startSide + pu + endSide);
    m_extentV = std::ceil(std::max(m_pv + m_shadow, laneV));

    const qreal alignedFillet = m_corner ? 0 : std::min<qreal>(m_fillet, m_offset);
    m_filletStart = m_align == Align::Start ? alignedFillet : m_fillet;
    m_filletEnd = m_align == Align::End ? alignedFillet : m_fillet;

    m_windowSize = horizontal() ? QSize(int(m_extentU), int(m_extentV)) : QSize(int(m_extentV), int(m_extentU));
    m_panelRect = rectToWindow(m_u0, 0, m_u1, m_pv);

    // The header sits on the side away from the edge (below the keys when docked to the top edge).
    const QRectF content = m_panelRect.adjusted(m_pad, m_pad, -m_pad, -m_pad);
    const qreal headerRow = m_header - m_gap;
    if (headerAtTop()) {
        m_headerRect = m_header > 0 ? QRectF(content.left(), content.top(), content.width(), headerRow) : QRectF();
        m_gridRect = QRectF(content.left(), content.top() + m_header, gridW, gridH);
    } else {
        m_gridRect = QRectF(content.left(), content.top(), gridW, gridH);
        m_headerRect = m_header > 0 ? QRectF(content.left(), m_gridRect.bottom() + m_gap, content.width(), headerRow)
                                    : QRectF();
    }

    m_fillPath = buildOutline(true);
    m_borderPath = buildOutline(false);
    m_backgroundRect = m_fillPath.boundingRect().adjusted(-m_shadow, -m_shadow, m_shadow, m_shadow);
    // Far enough that the shape (including a corner flare) and its shadow are fully behind the edge, plus
    // 30 % so the last frames of a slide-out are empty: compositors animate the final frame when the
    // surface unmaps (KWin's close effect), which would otherwise briefly show a sliver of the panel.
    m_slideVector = vectorToWindow(0, -1.3 * (m_pv + (m_corner ? m_fillet : 0) + m_shadow));
    m_inputRegion = QRegion(m_fillPath.toFillPolygon().toPolygon());
}

QPointF PanelGeometry::toastPosition(int slot, qreal width, bool besidePanel) const
{
    const qreal du = horizontal() ? width : m_toastHeight;
    const qreal dv = horizontal() ? m_toastHeight : width;
    const qreal step = (m_toastHeight + m_toastGap) * std::max(0, slot);
    const qreal v = m_toastMargin + (horizontal() ? step : 0);
    const qreal along = horizontal() ? 0 : step;

    qreal u = 0;
    if (besidePanel) {
        u = m_align == Align::Start ? m_u1 + m_laneGap + along : m_u0 - m_laneGap - along - du;
    } else {
        const qreal inset = m_corner ? m_toastMargin : 0;
        switch (m_align) {
        case Align::Start:
            u = m_u0 + inset + along;
            break;
        case Align::End:
            u = m_u1 - inset - along - du;
            break;
        case Align::Center:
            u = (m_u0 + m_u1 - du) / 2 - along;
            break;
        }
    }
    return rectToWindow(u, v, u + du, v + dv).topLeft();
}

QPointF PanelGeometry::toastSlideVector(qreal width) const
{
    const qreal dv = horizontal() ? m_toastHeight : width;
    return vectorToWindow(0, -(m_toastMargin + dv + std::round(16 * m_scale)));
}
