#pragma once

#include <QColor>
#include <QImage>
#include <QPointer>
#include <QQuickPaintedItem>
#include <QtQml/qqmlregistration.h>

class PanelGeometry;

// Paints the panel shape (fill + hairline along the non-flush sides) and its soft shadow. Place it at
// geometry.backgroundRect. The shadow is blurred once per geometry/DPR and knocked out under the
// panel so a translucent panel never looks muddy.
class PanelBackground : public QQuickPaintedItem
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(PanelGeometry *geometry READ geometry WRITE setGeometry NOTIFY geometryChanged)
    Q_PROPERTY(QColor panelColor READ panelColor WRITE setPanelColor NOTIFY panelColorChanged)
    Q_PROPERTY(QColor borderColor READ borderColor WRITE setBorderColor NOTIFY borderColorChanged)
    Q_PROPERTY(QColor shadowColor READ shadowColor WRITE setShadowColor NOTIFY shadowColorChanged)

public:
    explicit PanelBackground(QQuickItem *parent = nullptr);

    PanelGeometry *geometry() const { return m_geometry; }
    void setGeometry(PanelGeometry *geometry);
    QColor panelColor() const { return m_panelColor; }
    void setPanelColor(const QColor &color);
    QColor borderColor() const { return m_borderColor; }
    void setBorderColor(const QColor &color);
    QColor shadowColor() const { return m_shadowColor; }
    void setShadowColor(const QColor &color);

    void paint(QPainter *painter) override;

signals:
    void geometryChanged();
    void panelColorChanged();
    void borderColorChanged();
    void shadowColorChanged();

private:
    void invalidateShadow();
    QImage renderShadow(qreal dpr) const;

    QPointer<PanelGeometry> m_geometry;
    QColor m_panelColor;
    QColor m_borderColor;
    QColor m_shadowColor;
    QImage m_shadow;
    qreal m_shadowDpr = 0;
};
