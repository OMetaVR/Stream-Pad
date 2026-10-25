#pragma once

#include <QQuickImageProvider>

// "image://icon/<rrggbb|->/<percent-encoded path>": renders an icon file at the requested size.
// For SVGs, `currentColor` (lucide's stroke) is replaced with the given colour, since Qt's SVG renderer
// would otherwise paint it black; "-" draws the file as-is. Raster icons are scaled to fit.
class IconProvider : public QQuickImageProvider
{
public:
    IconProvider();

    QImage requestImage(const QString &id, QSize *size, const QSize &requestedSize) override;
};
