#pragma once

#include <QJsonArray>
#include <QObject>
#include <QTimer>

class QScreen;

// Describes the screens as OutputInfo[] (docs/SPEC.md §4) and reports changes, coalesced, via changed().
class OutputWatcher : public QObject
{
    Q_OBJECT

public:
    explicit OutputWatcher(QObject *parent = nullptr);

    static QJsonArray describe();

signals:
    void changed(const QJsonArray &outputs);

private:
    void watch(QScreen *screen);

    QTimer m_debounce;
};
