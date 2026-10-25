#include "steppedclock.h"

#include <climits>

SteppedClock::SteppedClock()
{
    install();
    m_keepAlive.setStartValue(0);
    m_keepAlive.setEndValue(1);
    m_keepAlive.setDuration(INT_MAX);
    m_keepAlive.start();
}

void SteppedClock::advanceTo(qint64 ms)
{
    m_now = ms;
    advanceAnimation();
}

void SteppedClock::start()
{
    m_startedAt = m_now;
    QAnimationDriver::start();
}
