#include "overlaysettings.h"
#include "panelgeometry.h"

#include <QJsonDocument>
#include <QTest>

namespace {

OverlaySettings layout(const QString &edge, const QString &align, int offset, qreal scale = 1.0, bool header = true)
{
    OverlaySettings s;
    s.edge = edge;
    s.align = align;
    s.offset = offset;
    s.scale = scale;
    s.showHeader = header;
    return s;
}

// Sample points along a path's outline (curves included).
QList<QPointF> outlinePoints(const QPainterPath &path)
{
    QList<QPointF> points;
    for (int i = 0; i <= 400; ++i)
        points << path.pointAtPercent(i / 400.0);
    return points;
}

bool onScreenEdge(const QPointF &p, PanelGeometry::Edge edge, const QSize &window)
{
    switch (edge) {
    case PanelGeometry::Edge::Bottom:
        return qAbs(p.y() - window.height()) < 0.01;
    case PanelGeometry::Edge::Top:
        return qAbs(p.y()) < 0.01;
    case PanelGeometry::Edge::Left:
        return qAbs(p.x()) < 0.01;
    case PanelGeometry::Edge::Right:
        return qAbs(p.x() - window.width()) < 0.01;
    }
    return false;
}

} // namespace

class TestPanelGeometry : public QObject
{
    Q_OBJECT

private slots:
    void layouts_data();
    void layouts();
    void cornerFlare();
    void alignedMargin();
    void scaleAndHeader();
    void settingsParsing();
};

void TestPanelGeometry::layouts_data()
{
    QTest::addColumn<QString>("edge");
    QTest::addColumn<QString>("align");
    QTest::addColumn<int>("offset");
    for (const char *edge : {"bottom", "top", "left", "right"}) {
        for (const char *align : {"start", "center", "end"}) {
            for (int offset : {0, 12, 80}) {
                QTest::addRow("%s-%s-%d", edge, align, offset) << QString::fromLatin1(edge) << QString::fromLatin1(align) << offset;
            }
        }
    }
}

void TestPanelGeometry::layouts()
{
    QFETCH(QString, edge);
    QFETCH(QString, align);
    QFETCH(int, offset);
    PanelGeometry g;
    g.setLayout(layout(edge, align, offset));

    const QRectF window(QPointF(0, 0), QSizeF(g.windowSize()));
    QVERIFY(window.contains(g.panelRect()));
    QVERIFY(window.adjusted(-0.01, -0.01, 0.01, 0.01).contains(g.fillPath().boundingRect()));
    QVERIFY(g.panelRect().contains(g.gridRect()));
    QVERIFY(g.panelRect().contains(g.headerRect()));
    QVERIFY(!g.gridRect().intersects(g.headerRect()));

    // Numpad body: 4×5 keys of 58 with 8 gaps and 14 padding, plus the 40 px header.
    QCOMPARE(g.panelRect().size(), QSizeF(284, 390));

    // The panel touches the screen edge, and its flush side carries no border.
    const QRectF p = g.panelRect();
    const QPointF flushMid = g.edge() == PanelGeometry::Edge::Bottom ? QPointF(p.center().x(), p.bottom())
                           : g.edge() == PanelGeometry::Edge::Top    ? QPointF(p.center().x(), p.top())
                           : g.edge() == PanelGeometry::Edge::Left   ? QPointF(p.left(), p.center().y())
                                                                     : QPointF(p.right(), p.center().y());
    QVERIFY(onScreenEdge(flushMid, g.edge(), g.windowSize()));
    const QList<QPointF> border = outlinePoints(g.borderPath());
    int onEdge = 0;
    for (const QPointF &p : border)
        onEdge += onScreenEdge(p, g.edge(), g.windowSize()) ? 1 : 0;
    QVERIFY2(onEdge <= 2, "the border must only touch the screen edge where the fillets end");

    // Input region = the shape: the panel centre is inside, the far window corners are not.
    QVERIFY(g.inputRegion().contains(g.panelRect().center().toPoint()));
    QVERIFY(!g.inputRegion().isEmpty());

    // Sliding by slideVector hides the shape (and its shadow area) completely behind the edge.
    const QRectF hiddenShape = g.backgroundRect().translated(g.slideVector());
    QVERIFY(!hiddenShape.intersects(window));

    // Toasts stay inside the window; beside the visible panel they never overlap it.
    for (int slot = 0; slot < 3; ++slot) {
        for (bool beside : {true, false}) {
            const QRectF toast(g.toastPosition(slot, g.toastMaxWidth(), beside), QSizeF(g.toastMaxWidth(), g.toastHeight()));
            QVERIFY2(window.contains(toast), qPrintable(QStringLiteral("slot %1 beside %2").arg(slot).arg(beside)));
            if (beside)
                QVERIFY(!toast.intersects(g.fillPath().boundingRect()));
            QVERIFY(!toast.translated(g.toastSlideVector(toast.width())).intersects(window) || slot > 0);
        }
    }
}

void TestPanelGeometry::cornerFlare()
{
    PanelGeometry g;
    g.setLayout(layout(QStringLiteral("bottom"), QStringLiteral("end"), 0));
    QVERIFY(g.cornerFlush());
    // Flush in the bottom-right corner, flaring 20 px up along the right screen edge.
    QCOMPARE(g.panelRect().right(), qreal(g.windowSize().width()));
    QCOMPARE(g.panelRect().bottom(), qreal(g.windowSize().height()));
    const QRectF shape = g.fillPath().boundingRect();
    QCOMPARE(shape.top(), g.panelRect().top() - 20);
    QCOMPARE(shape.right(), g.panelRect().right());
    QCOMPARE(shape.left(), g.panelRect().left() - 20);

    // The flare point on the right edge belongs to the shape; just inside the convex corner it does not.
    QVERIFY(g.fillPath().contains(QPointF(g.panelRect().right() - 1, g.panelRect().top() - 10)));
    QVERIFY(!g.fillPath().contains(QPointF(g.panelRect().right() - 5, g.panelRect().top() - 10)));
    QVERIFY(!g.fillPath().contains(QPointF(g.panelRect().left() + 1, g.panelRect().top() + 1)));

    g.setLayout(layout(QStringLiteral("left"), QStringLiteral("start"), 0));
    QCOMPARE(g.panelRect().topLeft(), QPointF(0, 0));
    QCOMPARE(g.fillPath().boundingRect().right(), g.panelRect().right() + 20);
}

void TestPanelGeometry::alignedMargin()
{
    PanelGeometry g;
    g.setLayout(layout(QStringLiteral("bottom"), QStringLiteral("end"), 100));
    // The window keeps 32 px for fillet + shadow on the aligned side; the margin covers the rest.
    QCOMPARE(g.alignedMargin(), 68);
    QCOMPARE(qreal(g.windowSize().width()) - g.panelRect().right(), 32.0);

    g.setLayout(layout(QStringLiteral("bottom"), QStringLiteral("end"), 10));
    QCOMPARE(g.alignedMargin(), 0);
    QCOMPARE(qreal(g.windowSize().width()) - g.panelRect().right(), 10.0);
    QVERIFY(!g.cornerFlush());

    g.setLayout(layout(QStringLiteral("top"), QStringLiteral("center"), 100));
    QCOMPARE(g.alignedMargin(), 0);
    QCOMPARE(g.panelRect().center().x(), g.windowSize().width() / 2.0);
}

void TestPanelGeometry::scaleAndHeader()
{
    PanelGeometry g;
    g.setLayout(layout(QStringLiteral("bottom"), QStringLiteral("end"), 0, 1.5, false));
    QCOMPARE(g.keyUnit(), 87.0);
    QCOMPARE(g.keyGap(), 12.0);
    QCOMPARE(g.headerRect(), QRectF());
    QCOMPARE(g.panelRect().size(), QSizeF(4 * 87 + 3 * 12 + 2 * 21, 5 * 87 + 4 * 12 + 2 * 21));

    g.setLayout(layout(QStringLiteral("top"), QStringLiteral("end"), 0));
    QVERIFY(!g.headerAtTop());
    QVERIFY(g.headerRect().top() > g.gridRect().bottom());
}

void TestPanelGeometry::settingsParsing()
{
    const auto parse = [](const char *json) {
        return OverlaySettings::fromJson(QJsonDocument::fromJson(json).object());
    };
    const OverlaySettings defaults = parse("{}");
    QCOMPARE(defaults, OverlaySettings());
    QCOMPARE(defaults.edge, QStringLiteral("bottom"));
    QCOMPARE(defaults.align, QStringLiteral("end"));
    QCOMPARE(defaults.opacity, 0.94);

    const OverlaySettings odd = parse(R"({"edge":"diagonal","align":"center","scale":9,"opacity":0.1,
        "offset":-5,"animationMs":99999,"accent":"#ff7a5980","legendStyle":"number","showLegends":false})");
    QCOMPARE(odd.edge, QStringLiteral("bottom"));
    QCOMPARE(odd.align, QStringLiteral("center"));
    QCOMPARE(odd.scale, 2.0);
    QCOMPARE(odd.opacity, 0.3);
    QCOMPARE(odd.offset, 0);
    QCOMPARE(odd.animationMs, 2000);
    QCOMPARE(odd.accent, QColor(0xff, 0x7a, 0x59, 0x80));
    QCOMPARE(odd.legendStyle, QStringLiteral("number"));
    QVERIFY(!odd.showLegends);
    QVERIFY(!odd.sameLayout(defaults));
}

QTEST_GUILESS_MAIN(TestPanelGeometry)
#include "tst_panelgeometry.moc"
