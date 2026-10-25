#pragma once

#include "overlaysettings.h"

#include <QObject>
#include <QPainterPath>
#include <QPointF>
#include <QRectF>
#include <QRegion>
#include <QSize>
#include <QtQml/qqmlregistration.h>

// Everything about where the overlay window sits and what the panel looks like geometrically
// (docs/SPEC.md §8 "Geometry" / "Shape"), in logical window coordinates.
//
// Internally every edge is laid out in one canonical frame: `u` runs along the screen edge from its
// start (left / top) to its end (right / bottom) and `v` points away from the edge into the screen.
// The window touches the screen edge at v = 0; the panel body spans [u0, u1] × [0, pv].
class PanelGeometry : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("Provided by Controller.geometry")
    Q_PROPERTY(QSize windowSize READ windowSize NOTIFY changed)
    Q_PROPERTY(QRectF panelRect READ panelRect NOTIFY changed)
    Q_PROPERTY(QRectF headerRect READ headerRect NOTIFY changed)
    Q_PROPERTY(QRectF gridRect READ gridRect NOTIFY changed)
    Q_PROPERTY(QRectF backgroundRect READ backgroundRect NOTIFY changed)
    Q_PROPERTY(QPointF slideVector READ slideVector NOTIFY changed)
    Q_PROPERTY(bool headerAtTop READ headerAtTop NOTIFY changed)
    Q_PROPERTY(bool verticalEdge READ verticalEdge NOTIFY changed)
    Q_PROPERTY(qreal scale READ scale NOTIFY changed)
    Q_PROPERTY(qreal keyUnit READ keyUnit NOTIFY changed)
    Q_PROPERTY(qreal keyGap READ keyGap NOTIFY changed)
    Q_PROPERTY(qreal keyRadius READ keyRadius NOTIFY changed)
    Q_PROPERTY(qreal toastHeight READ toastHeight NOTIFY changed)
    Q_PROPERTY(qreal toastMaxWidth READ toastMaxWidth NOTIFY changed)

public:
    enum class Edge { Bottom, Top, Left, Right };
    enum class Align { Start, Center, End };

    explicit PanelGeometry(QObject *parent = nullptr);

    // Recomputes from the layout-relevant settings; emits changed() only if something moved.
    void setLayout(const OverlaySettings &settings);

    QSize windowSize() const { return m_windowSize; }
    QRectF panelRect() const { return m_panelRect; }
    QRectF headerRect() const { return m_headerRect; }
    QRectF gridRect() const { return m_gridRect; }
    // Area the panel background item paints into: the shape plus room for its shadow.
    QRectF backgroundRect() const { return m_backgroundRect; }
    // Translation that moves the whole panel (shadow included) behind the screen edge.
    QPointF slideVector() const { return m_slideVector; }
    bool headerAtTop() const { return m_edge != Edge::Top; }
    bool verticalEdge() const { return !horizontal(); }
    qreal scale() const { return m_scale; }
    qreal keyUnit() const { return m_unit; }
    qreal keyGap() const { return m_gap; }
    qreal keyRadius() const { return m_keyRadius; }
    qreal shadowExtent() const { return m_shadow; }
    qreal toastHeight() const { return m_toastHeight; }
    qreal toastMaxWidth() const { return m_toastMaxWidth; }

    // Rest position (top-left) of toast `slot` (0 = newest) of the given width. `besidePanel` places the
    // stack next to the visible panel; otherwise the toasts stand in for the panel at its position.
    Q_INVOKABLE QPointF toastPosition(int slot, qreal width, bool besidePanel) const;
    // Translation that hides a toast of the given width behind the screen edge.
    Q_INVOKABLE QPointF toastSlideVector(qreal width) const;

    // Closed outline of the panel including the edge fillets.
    const QPainterPath &fillPath() const { return m_fillPath; }
    // The non-flush part of the outline (where the hairline border is drawn).
    const QPainterPath &borderPath() const { return m_borderPath; }
    QRegion inputRegion() const { return m_inputRegion; }

    Edge edge() const { return m_edge; }
    Align align() const { return m_align; }
    // Distance between the window and the screen corner on the aligned side (start/end only).
    int alignedMargin() const { return m_alignedMargin; }
    // Aligned to a screen corner with offset 0: the panel also flares into the adjacent edge.
    bool cornerFlush() const { return m_corner; }

    static Edge parseEdge(const QString &edge);
    static Align parseAlign(const QString &align);

signals:
    void changed();

private:
    bool horizontal() const { return m_edge == Edge::Bottom || m_edge == Edge::Top; }
    QPointF toWindow(qreal u, qreal v) const;
    QRectF rectToWindow(qreal u0, qreal v0, qreal u1, qreal v1) const;
    QPointF vectorToWindow(qreal du, qreal dv) const;
    QPainterPath buildOutline(bool closed) const;
    void recompute();

    // Inputs
    Edge m_edge = Edge::Bottom;
    Align m_align = Align::End;
    int m_offset = 0;
    qreal m_scale = 1.0;
    bool m_showHeader = true;
    bool m_initialized = false;

    // Metrics (logical px, already scaled)
    qreal m_unit = 0, m_gap = 0, m_pad = 0, m_header = 0;
    qreal m_keyRadius = 0, m_outerRadius = 0, m_fillet = 0, m_shadow = 0;
    qreal m_toastHeight = 0, m_toastGap = 0, m_toastMaxWidth = 0, m_toastMargin = 0, m_laneGap = 0;

    // Canonical layout
    qreal m_extentU = 0, m_extentV = 0;
    qreal m_u0 = 0, m_u1 = 0, m_pv = 0;
    qreal m_filletStart = 0, m_filletEnd = 0;
    bool m_corner = false;
    int m_alignedMargin = 0;

    // Window-space results
    QSize m_windowSize;
    QRectF m_panelRect, m_headerRect, m_gridRect, m_backgroundRect;
    QPointF m_slideVector;
    QPainterPath m_fillPath, m_borderPath;
    QRegion m_inputRegion;
};
