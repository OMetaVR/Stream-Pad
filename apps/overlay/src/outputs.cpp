#include "outputs.h"

#include <QGuiApplication>
#include <QJsonObject>
#include <QScreen>

using namespace Qt::StringLiterals;

OutputWatcher::OutputWatcher(QObject *parent) : QObject(parent)
{
    m_debounce.setSingleShot(true);
    m_debounce.setInterval(100);
    connect(&m_debounce, &QTimer::timeout, this, [this] { emit changed(describe()); });

    const auto schedule = [this] { m_debounce.start(); };
    connect(qGuiApp, &QGuiApplication::screenAdded, this, [this, schedule](QScreen *screen) {
        watch(screen);
        schedule();
    });
    connect(qGuiApp, &QGuiApplication::screenRemoved, this, schedule);
    connect(qGuiApp, &QGuiApplication::primaryScreenChanged, this, schedule);
    for (QScreen *screen : QGuiApplication::screens())
        watch(screen);
}

void OutputWatcher::watch(QScreen *screen)
{
    const auto schedule = [this] { m_debounce.start(); };
    connect(screen, &QScreen::geometryChanged, this, schedule);
    connect(screen, &QScreen::logicalDotsPerInchChanged, this, schedule);
    connect(screen, &QScreen::physicalDotsPerInchChanged, this, schedule);
}

QJsonArray OutputWatcher::describe()
{
    QJsonArray outputs;
    const QScreen *primary = QGuiApplication::primaryScreen();
    for (const QScreen *screen : QGuiApplication::screens()) {
        const QRect g = screen->geometry();
        const QString model = screen->model().trimmed();
        outputs.append(QJsonObject{
            {u"name"_s, screen->name()},
            {u"x"_s, g.x()},
            {u"y"_s, g.y()},
            {u"width"_s, g.width()},
            {u"height"_s, g.height()},
            {u"scale"_s, screen->devicePixelRatio()},
            {u"primary"_s, screen == primary},
            {u"model"_s, model.isEmpty() ? QJsonValue(QJsonValue::Null) : QJsonValue(model)},
        });
    }
    return outputs;
}
