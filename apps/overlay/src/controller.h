#pragma once

#include "panelgeometry.h"

#include <QObject>
#include <QPoint>
#include <QPointer>
#include <QRegion>
#include <QString>
#include <QtQml/qqmlregistration.h>

class OverlayStore;
class QQmlEngine;
class QJSEngine;
class QQuickWindow;
class QScreen;

// Owns the overlay window's lifecycle (docs/SPEC.md §8): which screen it is on, layer-shell anchoring,
// mapping it only while something is shown, the input region and KWin blur.
//
// QML animates `progress` (0 = panel behind the screen edge, 1 = at rest) towards `presented`;
// the window is unmapped once the panel is fully hidden and no toast remains.
class OverlayController : public QObject
{
    Q_OBJECT
    QML_NAMED_ELEMENT(Controller)
    QML_SINGLETON
    Q_PROPERTY(PanelGeometry *geometry READ geometry CONSTANT)
    Q_PROPERTY(bool presented READ presented NOTIFY presentedChanged)
    Q_PROPERTY(qreal progress READ progress WRITE setProgress NOTIFY progressChanged)
    // Mapped and the first frame is on screen: animations may start.
    Q_PROPERTY(bool ready READ ready NOTIFY readyChanged)
    Q_PROPERTY(bool toastsActive READ toastsActive WRITE setToastsActive NOTIFY toastsActiveChanged)

public:
    explicit OverlayController(OverlayStore *store, QObject *parent = nullptr);
    ~OverlayController() override;

    static OverlayController *create(QQmlEngine *, QJSEngine *engine);

    void attach(QQuickWindow *window);
    // Screenshot mode: present the panel regardless of the daemon's visibility.
    void setForceVisible(bool force);

    PanelGeometry *geometry() { return &m_geometry; }
    bool presented() const { return m_presented; }
    qreal progress() const { return m_progress; }
    void setProgress(qreal progress);
    bool ready() const { return m_ready; }
    bool toastsActive() const { return m_toastsActive; }
    void setToastsActive(bool active);

signals:
    void presentedChanged();
    void progressChanged();
    void readyChanged();
    void toastsActiveChanged();

private:
    bool wantPanel() const;
    void sync();
    void scheduleSync();
    void map();
    void unmap();
    void unmapAfterEmptyFrame();
    void onFirstFrame();
    void onWindowHidden();
    void setPresented(bool presented);
    void setReady(bool ready);
    void chooseScreen();
    void applyPlacement();
    void applyInputRegion();
    void applyBlur();
    QPoint slideOffset() const;

    OverlayStore *m_store;
    PanelGeometry m_geometry;
    QPointer<QQuickWindow> m_window;
    bool m_layerShell = false;
    bool m_forceVisible = false;
    bool m_mapped = false;
    bool m_mappedForPanel = false; // false while the window is up only for toasts
    bool m_awaitingFirstFrame = false;
    bool m_ready = false;
    bool m_presented = false;
    bool m_toastsActive = false;
    bool m_syncQueued = false;
    bool m_unmapPending = false;
    qreal m_progress = 0;
    QString m_mappedOutput;
    bool m_blurApplied = false;
    QRegion m_blurRegion;
};
