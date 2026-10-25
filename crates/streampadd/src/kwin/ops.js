// One-shot queries and window/desktop commands. Runs `SP_OPS[SP.op](SP.args)` and answers through
// `Reply(token, {"ok": result} | {"error": message})`.

function spIsAppWindow(w) {
    return !!w && !w.deleted && !w.specialWindow && (w.normalWindow || w.dialog) && !w.skipTaskbar;
}

// Topmost first.
function spAppWindows() {
    return workspace.stackingOrder.filter(spIsAppWindow).reverse();
}

function spTargetWindow() {
    var w = workspace.activeWindow;
    if (!w || w.deleted || w.specialWindow) throw new Error("no active window");
    return w;
}

// Condition "activeApp" semantics: comma-separated, case-insensitive substrings, or a "/regex/flags".
function spMatcher(pattern) {
    var p = String(pattern).trim();
    var end = p.lastIndexOf("/");
    if (p.length > 2 && p.charAt(0) === "/" && end > 0) {
        var re = new RegExp(p.slice(1, end), p.slice(end + 1) || "i");
        return function (s) { return re.test(String(s)); };
    }
    var parts = p.toLowerCase().split(",").map(function (x) { return x.trim(); }).filter(function (x) { return x.length > 0; });
    return function (s) {
        s = String(s).toLowerCase();
        return parts.some(function (x) { return s.indexOf(x) >= 0; });
    };
}

/** Windows whose desktop file / resource class / resource name equals one of `ids` (case-insensitive). */
function spAppIs(ids) {
    var wanted = ids.map(function (x) { return String(x).toLowerCase(); });
    return function (w) {
        return [w.desktopFileName, w.resourceClass, w.resourceName].some(function (v) {
            return !!v && wanted.indexOf(String(v).toLowerCase()) >= 0;
        });
    };
}

function spActivate(w) {
    if (w.minimized) w.minimized = false;
    workspace.activeWindow = w;
    return true;
}

// Focus the most recent candidate; if one is already active, go to the least recent so repeated presses cycle
// through all of an app's windows.
function spFocusAmong(cands) {
    if (cands.length === 0) return false;
    var active = workspace.activeWindow;
    if (cands.indexOf(active) < 0) return spActivate(cands[0]);
    return cands.length === 1 ? true : spActivate(cands[cands.length - 1]);
}

function spCenter(w) {
    var area = workspace.clientArea(KWin.MaximizeArea, w.output, workspace.currentDesktop);
    var g = w.frameGeometry;
    var width = Math.min(g.width, area.width), height = Math.min(g.height, area.height);
    w.frameGeometry = {
        x: Math.round(area.x + (area.width - width) / 2), y: Math.round(area.y + (area.height - height) / 2),
        width: width, height: height
    };
}

function spAdjacentOutput(from, dir) {
    var g = from.geometry, cx = g.x + g.width / 2, cy = g.y + g.height / 2, best = null, bestDist = Infinity;
    workspace.screens.forEach(function (o) {
        if (o === from) return;
        var h = o.geometry, dx = h.x + h.width / 2 - cx, dy = h.y + h.height / 2 - cy;
        var horizontal = Math.abs(dx) >= Math.abs(dy);
        var ok = dir === "left" ? dx < 0 && horizontal
            : dir === "right" ? dx > 0 && horizontal
            : dir === "up" ? dy < 0 && !horizontal
            : dy > 0 && !horizontal;
        var dist = dx * dx + dy * dy;
        if (ok && dist < bestDist) { best = o; bestDist = dist; }
    });
    return best;
}

function spOutputTarget(w, target) {
    var screens = workspace.screens, n = screens.length, i = screens.indexOf(w.output);
    switch (target) {
        case "next": return screens[(i + 1) % n];
        case "prev": return screens[(i - 1 + n) % n];
        case "left": case "right": case "up": case "down": return spAdjacentOutput(w.output, target);
    }
    for (var k = 0; k < n; k++) if (String(screens[k].name) === target) return screens[k];
    throw new Error("no monitor named " + target);
}

function spDesktopIndex() {
    return workspace.desktops.indexOf(workspace.currentDesktop);
}

function spBottomOfColumn(col, cols, n) {
    var i = Math.floor((n - 1) / cols) * cols + col;
    return i < n ? i : i - cols;
}

// Resolves "next" | "prev" | "left" | "right" | "up" | "down" | 1-based number. Null = no desktop that way.
function spDesktopTarget(target) {
    var ds = workspace.desktops, n = ds.length, cur = spDesktopIndex();
    if (typeof target === "number") {
        if (target < 1 || target > n || Math.floor(target) !== target) throw new Error("there is no desktop " + target);
        return ds[target - 1];
    }
    var wrap = workspace.virtualDesktopNavigationWrapsAround;
    var cols = Math.max(1, workspace.desktopGridWidth), row = Math.floor(cur / cols), col = cur % cols;
    var i = -1;
    switch (target) {
        case "next": i = cur + 1 < n ? cur + 1 : (wrap ? 0 : -1); break;
        case "prev": i = cur > 0 ? cur - 1 : (wrap ? n - 1 : -1); break;
        case "left": i = col > 0 ? cur - 1 : (wrap ? Math.min(row * cols + cols - 1, n - 1) : -1); break;
        case "right": i = col + 1 < cols && cur + 1 < n ? cur + 1 : (wrap ? row * cols : -1); break;
        case "up": i = row > 0 ? cur - cols : (wrap ? spBottomOfColumn(col, cols, n) : -1); break;
        case "down": i = cur + cols < n ? cur + cols : (wrap ? col : -1); break;
        default: throw new Error("unknown desktop target " + target);
    }
    return i >= 0 && i !== cur ? ds[i] : null;
}

var SP_OPS = {
    windows: function () { return spAppWindows().map(spWindow); },
    activeWindow: function () { return spWindow(workspace.activeWindow); },
    outputs: function () { return workspace.screens.map(spOutput); },
    cursorOutput: function () {
        var o = workspace.screenAt(workspace.cursorPos);
        return o ? String(o.name) : null;
    },
    focus: function (a) {
        var m = spMatcher(a.match);
        return spFocusAmong(spAppWindows().filter(function (w) {
            return a.by === "title" ? m(w.caption) : (m(w.resourceName) || m(w.resourceClass) || m(w.desktopFileName));
        }));
    },
    focusApp: function (a) {
        return spFocusAmong(spAppWindows().filter(spAppIs(a.ids)));
    },
    closeApp: function (a) {
        var pids = [];
        spAppWindows().filter(spAppIs(a.ids)).forEach(function (w) {
            if (w.pid > 0 && pids.indexOf(w.pid) < 0) pids.push(w.pid);
            w.closeWindow();
        });
        return pids;
    },
    windowAction: function (a) {
        var w = spTargetWindow();
        switch (a.action) {
            case "minimize": w.minimized = true; break;
            case "maximize": var full = w.maximizeMode === 3; w.setMaximize(!full, !full); break;
            case "fullscreen": w.fullScreen = !w.fullScreen; break;
            case "close": w.closeWindow(); break;
            case "keepAbove": w.keepAbove = !w.keepAbove; break;
            case "center": spCenter(w); break;
            default: throw new Error("unknown window action " + a.action);
        }
        return null;
    },
    moveToOutput: function (a) {
        var w = spTargetWindow(), o = spOutputTarget(w, a.target);
        if (o && o !== w.output) workspace.sendClientToScreen(w, o);
        return o ? String(o.name) : null;
    },
    desktopSwitch: function (a) {
        var d = spDesktopTarget(a.target);
        if (d) workspace.currentDesktop = d;
        return null;
    },
    desktopMoveWindow: function (a) {
        var w = spTargetWindow(), d = spDesktopTarget(a.target);
        if (!d) return null;
        w.desktops = [d];
        if (a.follow) {
            workspace.currentDesktop = d;
            workspace.activeWindow = w;
        }
        return null;
    },
    desktopCreate: function () {
        var n = workspace.desktops.length;
        workspace.createDesktop(n, "Desktop " + (n + 1));
        var ds = workspace.desktops;
        if (ds.length > n) workspace.currentDesktop = ds[n];
        return null;
    },
    desktopRemove: function () {
        if (workspace.desktops.length <= 1) throw new Error("can't remove the only desktop");
        workspace.removeDesktop(workspace.currentDesktop);
        return null;
    },
    desktopCurrent: function () {
        var d = workspace.currentDesktop;
        return { index: spDesktopIndex() + 1, name: String(d.name), count: workspace.desktops.length };
    }
};

(function () {
    var answer;
    try {
        var op = SP_OPS[SP.op];
        if (!op) throw new Error("unknown operation " + SP.op);
        var result = op(SP.args);
        answer = { ok: result === undefined ? null : result };
    } catch (e) {
        answer = { error: String(e && e.message ? e.message : e) };
    }
    callDBus(SP.service, SP.path, SP.iface, "Reply", SP.token, JSON.stringify(answer));
})();
