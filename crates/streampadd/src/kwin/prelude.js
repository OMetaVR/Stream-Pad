// Shared by every StreamPad KWin script. The daemon prepends `var SP = { service, path, iface, token, op, args }`.

function spPrimaryName() {
    var order = workspace.screenOrder;
    if (order && order.length) return String(order[0].name);
    return workspace.screens.length ? String(workspace.screens[0].name) : null;
}

function spOutput(o) {
    var g = o.geometry;
    return {
        name: String(o.name), x: g.x, y: g.y, width: g.width, height: g.height,
        scale: o.devicePixelRatio, primary: String(o.name) === spPrimaryName(),
        model: o.model ? String(o.model) : null
    };
}

function spWindow(w) {
    if (!w) return null;
    return {
        id: String(w.internalId), title: String(w.caption || ""), appId: String(w.resourceName || ""),
        resourceClass: String(w.resourceClass || ""), desktopFile: String(w.desktopFileName || ""),
        pid: w.pid || 0, output: w.output ? String(w.output.name) : null, active: !!w.active
    };
}
