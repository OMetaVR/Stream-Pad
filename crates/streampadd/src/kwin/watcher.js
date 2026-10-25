// Persistent watcher: reports the active window (incl. title changes) and the output layout through
// `Event(json)`, so `window.active()` and output lookups never wait for a script round trip.

var spCurrent = null;

function spEmit(e) {
    callDBus(SP.service, SP.path, SP.iface, "Event", JSON.stringify(e));
}

function spReportActive() {
    spEmit({ type: "activeWindow", window: spWindow(spCurrent) });
}

function spTrack(w) {
    if (spCurrent) {
        try { spCurrent.captionChanged.disconnect(spReportActive); } catch (e) { /* window already gone */ }
    }
    spCurrent = w || null;
    if (spCurrent) spCurrent.captionChanged.connect(spReportActive);
    spReportActive();
}

function spReportOutputs() {
    spEmit({ type: "outputs", outputs: workspace.screens.map(spOutput) });
}

workspace.windowActivated.connect(spTrack);
workspace.screensChanged.connect(spReportOutputs);
workspace.virtualScreenGeometryChanged.connect(spReportOutputs);
if (workspace.screenOrderChanged) workspace.screenOrderChanged.connect(spReportOutputs);
spTrack(workspace.activeWindow);
spReportOutputs();
