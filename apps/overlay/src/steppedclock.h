#pragma once

#include <QAnimationDriver>
#include <QVariantAnimation>

// The animation clock of `--capture`: replaces Qt's wall-clock driver so that every QML Animation,
// Behavior, SpringAnimation and Timer (QML timers run on animation jobs) only moves when advanceTo()
// is called, and then by exactly the virtual time that passed.
class SteppedClock : public QAnimationDriver
{
public:
    SteppedClock();

    // Sets the virtual time (ms since the start of the capture) and runs everything that is due.
    void advanceTo(qint64 ms);

    // Qt expects a driver's elapsed() to count from the moment the driver was started.
    qint64 elapsed() const override { return m_now - m_startedAt; }

protected:
    void start() override;

private:
    // Qt hands over to a real-time timer whenever the only animations left are pure waits (a QML Timer or
    // PauseAnimation), and stops the driver. This endless animation keeps the driver running instead.
    QVariantAnimation m_keepAlive;
    qint64 m_now = 0;
    qint64 m_startedAt = 0;
};
