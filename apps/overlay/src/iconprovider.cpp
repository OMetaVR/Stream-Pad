#include "iconprovider.h"

#include <QColor>
#include <QFile>
#include <QImageReader>
#include <QLoggingCategory>
#include <QPainter>
#include <QSvgRenderer>
#include <QUrl>

Q_LOGGING_CATEGORY(lcIcons, "streampad.overlay.icons", QtInfoMsg)

namespace {

constexpr qsizetype kMaxSvgBytes = 4 * 1024 * 1024;
constexpr int kDefaultSize = 64;

QSize targetSize(const QSize &requested, const QSize &natural)
{
    if (requested.width() > 0 && requested.height() > 0)
        return requested;
    if (requested.width() > 0)
        return {requested.width(), requested.width()};
    if (requested.height() > 0)
        return {requested.height(), requested.height()};
    return natural.isValid() && !natural.isEmpty() ? natural : QSize(kDefaultSize, kDefaultSize);
}

QImage renderSvg(const QString &path, const QString &color, const QSize &requested)
{
    QFile file(path);
    if (!file.open(QIODevice::ReadOnly))
        return {};
    QByteArray svg = file.read(kMaxSvgBytes);
    if (!color.isEmpty())
        svg.replace("currentColor", color.toLatin1());

    QSvgRenderer renderer(svg);
    if (!renderer.isValid())
        return {};
    const QSize size = targetSize(requested, renderer.defaultSize());
    QImage image(size, QImage::Format_ARGB32_Premultiplied);
    image.fill(Qt::transparent);
    QPainter painter(&image);
    painter.setRenderHint(QPainter::Antialiasing);
    QSizeF fitted = renderer.viewBoxF().size().scaled(QSizeF(size), Qt::KeepAspectRatio);
    if (fitted.isEmpty())
        fitted = QSizeF(size);
    const QPointF origin((size.width() - fitted.width()) / 2, (size.height() - fitted.height()) / 2);
    renderer.render(&painter, QRectF(origin, fitted));
    return image;
}

QImage readRaster(const QString &path, const QSize &requested)
{
    QImageReader reader(path);
    reader.setAutoTransform(true);
    const QSize natural = reader.size();
    if (natural.isValid()) {
        const QSize box = targetSize(requested, natural);
        reader.setScaledSize(natural.scaled(box, Qt::KeepAspectRatio));
    }
    return reader.read();
}

} // namespace

IconProvider::IconProvider() : QQuickImageProvider(QQuickImageProvider::Image) {}

QImage IconProvider::requestImage(const QString &id, QSize *size, const QSize &requestedSize)
{
    const qsizetype slash = id.indexOf(u'/');
    if (slash <= 0)
        return {};
    const QString tint = id.left(slash);
    const QString path = QUrl::fromPercentEncoding(id.mid(slash + 1).toUtf8());

    QString color;
    if (tint != u"-") {
        const QColor parsed = QColor::fromString(u'#' + tint);
        if (parsed.isValid())
            color = parsed.name(QColor::HexRgb);
    }

    const bool svg = path.endsWith(u".svg", Qt::CaseInsensitive);
    const QImage image = svg ? renderSvg(path, color, requestedSize) : readRaster(path, requestedSize);
    if (image.isNull())
        qCWarning(lcIcons) << "cannot load icon" << path;
    if (size)
        *size = image.size();
    return image;
}
