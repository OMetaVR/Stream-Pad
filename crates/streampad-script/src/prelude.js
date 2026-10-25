// StreamPad macro API (docs/SPEC.md §6, crates/streampad-script/API.md) built on the native bridge:
// __host(method, argsJson) -> resultJson, sleep(ms) and __node(id). Evaluated at the start of every run,
// before the shared library and the macro, to an init function the runtime calls with the length of the
// macro wrapper's line-1 prefix (src/source.rs). Every global it defines is read-only and the namespace
// objects are frozen, so user code cannot replace them by accident. Init returns the runtime's private
// control object (see the end of this file).
(macroPrefixLength) => {
  "use strict";

  const host = __host;
  const nativeSleep = sleep;
  const traceNode = __node;

  const call = (method, args) => JSON.parse(host(method, JSON.stringify(args ?? {})));

  // ---- argument checks ----------------------------------------------------------------------------------

  const fail = (fn, message) => {
    throw new TypeError(`${fn}: ${message}`);
  };

  const show = (v) => (typeof v === "string" ? JSON.stringify(v) : inspect(v));

  const string = (fn, what, v) => {
    if (typeof v !== "string") fail(fn, `expected ${what} as a string, got ${show(v)}`);
    if (v.trim() === "") fail(fn, `${what} is empty`);
    return v;
  };

  const options = (fn, opts) => {
    if (opts === undefined || opts === null) return {};
    if (typeof opts !== "object") fail(fn, `options must be an object, got ${show(opts)}`);
    return opts;
  };

  const oneOf = (fn, what, v, allowed) => {
    if (!allowed.includes(v)) fail(fn, `${what} must be one of ${allowed.map((a) => JSON.stringify(a)).join(", ")}, got ${show(v)}`);
    return v;
  };

  const millis = (fn, what, v, fallback) => {
    if (v === undefined) return fallback;
    if (typeof v !== "number" || !(v >= 0) || !Number.isFinite(v)) fail(fn, `${what} must be a number of milliseconds (0 or more), got ${show(v)}`);
    return v;
  };

  const text = (v) => (typeof v === "string" ? v : inspect(v));

  // ---- pretty printing (log, toast, notify) -------------------------------------------------------------

  const FRAME = /^\s*at (?:(.+) \()?(macro|library|prelude):(\d+):(\d+)\)?$/;

  // A QuickJS stack in the form the runtime reports errors (src/errors.rs): positions in the user's code
  // ("line 3:5", "library line 2:1"), without the prelude's own frames.
  const userStack = (stack) =>
    stack
      .split("\n")
      .flatMap((frame) => {
        const m = FRAME.exec(frame);
        if (!m) return frame.trim() ? [frame] : [];
        const [, fn, file, line, col] = m;
        if (file === "prelude") return [];
        const column = file === "macro" && line === "1" ? Math.max(1, Number(col) - macroPrefixLength) : Number(col);
        const where = `${file === "library" ? "library line" : "line"} ${line}:${column}`;
        return [`    at ${fn ? `${fn} (${where})` : where}`];
      })
      .join("\n");

  const IDENTIFIER = /^[A-Za-z_$][\w$]*$/;
  const MAX_DEPTH = 4;
  const MAX_ITEMS = 100;
  const MAX_INLINE = 72;

  function inspect(value, depth = 0, seen = new Set()) {
    switch (typeof value) {
      case "string":
        return depth === 0 ? value : JSON.stringify(value);
      case "bigint":
        return `${value}n`;
      case "symbol":
        return value.toString();
      case "function":
        return `[Function ${value.name || "(anonymous)"}]`;
      case "object":
        break;
      default:
        return Object.is(value, -0) ? "-0" : String(value);
    }
    if (value === null) return "null";
    if (value instanceof Error) {
      const head = value.message ? `${value.name}: ${value.message}` : value.name;
      const stack = typeof value.stack === "string" ? userStack(value.stack) : "";
      return stack ? `${head}\n${stack}` : head;
    }
    if (value instanceof Date) return Number.isNaN(value.getTime()) ? "Invalid Date" : value.toISOString();
    if (value instanceof RegExp) return String(value);
    if (seen.has(value)) return "[Circular]";
    if (depth >= MAX_DEPTH) return Array.isArray(value) ? "[Array]" : "[Object]";

    seen.add(value);
    try {
      const nested = (v) => inspect(v, depth + 1, seen);
      // The first MAX_ITEMS entries of a collection, formatted, plus a count of the rest.
      const entries = (list, format) => {
        const shown = list.slice(0, MAX_ITEMS).map(format);
        if (list.length > MAX_ITEMS) shown.push(`… ${list.length - MAX_ITEMS} more`);
        return shown;
      };
      let open, close, items;
      if (Array.isArray(value)) {
        [open, close, items] = ["[", "]", entries(value, nested)];
      } else if (value instanceof Map) {
        [open, close, items] = [`Map(${value.size}) {`, "}", entries([...value], ([k, v]) => `${nested(k)} => ${nested(v)}`)];
      } else if (value instanceof Set) {
        [open, close, items] = [`Set(${value.size}) {`, "}", entries([...value], nested)];
      } else {
        const key = (k) => (IDENTIFIER.test(k) ? k : JSON.stringify(k));
        [open, close, items] = ["{", "}", entries(Object.keys(value), (k) => `${key(k)}: ${nested(value[k])}`)];
      }
      if (items.length === 0) return open.endsWith("{") && open !== "{" ? `${open}}` : open + close;
      const pad = open === "[" ? "" : " ";
      const inline = `${open}${pad}${items.join(", ")}${pad}${close}`;
      if (inline.length <= MAX_INLINE && !inline.includes("\n")) return inline;
      const indent = "  ".repeat(depth + 1);
      return `${open}\n${indent}${items.join(`,\n${indent}`)}\n${"  ".repeat(depth)}${close}`;
    } finally {
      seen.delete(value);
    }
  }

  // ---- window matching (Condition "activeApp" / "activeTitle" / "appRunning") ---------------------------

  const REGEX_FORM = /^\/(.+)\/([a-z]*)$/s;

  // "firefox, chromium": case-insensitive substring of any item; "/^org\.kde\./": a regular expression,
  // case-insensitive unless flags are given. Matched against appId / resourceClass / desktopFile, or the
  // title with { by: "title" }. The daemon's KWin script uses the same rules for window.focus.
  const windowMatcher = (fn, pattern, opts) => {
    string(fn, "match", pattern);
    const by = oneOf(fn, "by", options(fn, opts).by ?? "app", ["app", "title"]);
    const fields = by === "title" ? (w) => [w.title] : (w) => [w.appId, w.resourceClass, w.desktopFile];
    const regex = REGEX_FORM.exec(pattern.trim());
    let test;
    if (regex) {
      let re;
      try {
        re = new RegExp(regex[1], regex[2] || "i");
      } catch (e) {
        fail(fn, `invalid regular expression ${pattern.trim()}: ${e.message}`);
      }
      test = (field) => {
        re.lastIndex = 0;
        return re.test(field);
      };
    } else {
      const needles = pattern.split(",").map((s) => s.trim().toLowerCase()).filter(Boolean);
      test = (field) => needles.some((n) => field.toLowerCase().includes(n));
    }
    return (w) => w != null && fields(w).some((f) => typeof f === "string" && f !== "" && test(f));
  };

  // ---- the API ------------------------------------------------------------------------------------------

  function press(...combos) {
    if (combos.length === 0) fail("press", 'expected at least one shortcut, e.g. press("ctrl+c")');
    for (const combo of combos) call("key.press", { combo: string("press", "a shortcut", combo) });
  }

  function keyDown(combo) {
    call("key.down", { combo: string("keyDown", "a shortcut", combo) });
  }

  function keyUp(combo) {
    call("key.up", { combo: string("keyUp", "a shortcut", combo) });
  }

  function type(value, opts) {
    if (value === undefined || value === null) fail("type", `expected the text to type, got ${show(value)}`);
    call("key.type", { text: text(value), delayMs: millis("type", "delayMs", options("type", opts).delayMs, 8) });
  }

  const MEDIA_ACTIONS = ["playPause", "next", "prev", "stop", "volumeUp", "volumeDown", "mute"];

  function media(action) {
    call("key.media", { action: oneOf("media", "action", action, MEDIA_ACTIONS) });
  }

  function launch(app, opts) {
    const o = options("launch", opts);
    call("app.launch", {
      app: string("launch", "an app", app),
      focusIfRunning: o.focusIfRunning !== false,
      args: o.args === undefined || o.args === null ? "" : String(o.args),
    });
  }

  function closeApp(app, opts) {
    const o = options("closeApp", opts);
    return call("app.close", { app: string("closeApp", "an app", app), force: o.force === true }) === true;
  }

  function openUrl(url) {
    call("open.url", { url: string("openUrl", "a URL", url) });
  }

  function openPath(path) {
    call("open.path", { path: string("openPath", "a path", path) });
  }

  function run(command, opts) {
    const result = call("run.command", { command: string("run", "a command", command), wait: Boolean(options("run", opts).wait) });
    return result ?? undefined;
  }

  const windowAction = (action) =>
    function () {
      call("window.action", { action });
    };

  const window = Object.freeze({
    active() {
      return call("window.active") ?? null;
    },
    list() {
      return call("window.list") ?? [];
    },
    isActive(match, opts) {
      const matches = windowMatcher("window.isActive", match, opts);
      return matches(call("window.active"));
    },
    isRunning(match, opts) {
      const matches = windowMatcher("window.isRunning", match, opts);
      return (call("window.list") ?? []).some(matches);
    },
    focus(match, opts) {
      const by = oneOf("window.focus", "by", options("window.focus", opts).by ?? "app", ["app", "title"]);
      return call("window.focus", { match: string("window.focus", "match", match), by }) === true;
    },
    moveToOutput(target) {
      call("window.moveToOutput", { target: string("window.moveToOutput", "a target", target) });
    },
    minimize: windowAction("minimize"),
    maximize: windowAction("maximize"),
    fullscreen: windowAction("fullscreen"),
    close: windowAction("close"),
    keepAbove: windowAction("keepAbove"),
    center: windowAction("center"),
  });

  const desktopTarget = (fn, target) => {
    const ok = typeof target === "number" ? Number.isInteger(target) && target >= 1 : typeof target === "string" && target.trim() !== "";
    if (!ok) fail(fn, `target must be "next", "prev", "left", "right", "up", "down" or a desktop number from 1, got ${show(target)}`);
    return target;
  };

  const desktop = Object.freeze({
    switchTo(target) {
      call("desktop.switch", { target: desktopTarget("desktop.switchTo", target) });
    },
    moveWindow(target, opts) {
      call("desktop.moveWindow", {
        target: desktopTarget("desktop.moveWindow", target),
        follow: Boolean(options("desktop.moveWindow", opts).follow),
      });
    },
    create() {
      call("desktop.create");
    },
    remove() {
      call("desktop.remove");
    },
    overview() {
      call("desktop.overview");
    },
    current() {
      return call("desktop.current");
    },
  });

  const kde = Object.freeze({
    shortcut(component, name) {
      call("kde.shortcut", { component: string("kde.shortcut", "a component", component), name: string("kde.shortcut", "a shortcut name", name) });
    },
  });

  const profile = Object.freeze({
    next() {
      call("profile.set", { target: "next" });
    },
    prev() {
      call("profile.set", { target: "prev" });
    },
    set(target) {
      const ok = typeof target === "number" ? Number.isInteger(target) && target >= 0 : typeof target === "string" && target.trim() !== "";
      if (!ok) fail("profile.set", `expected a profile id, name or index, got ${show(target)}`);
      call("profile.set", { target });
    },
    current() {
      return call("profile.current");
    },
  });

  const macroMode = Object.freeze({
    get() {
      return call("macroMode.get") === true;
    },
    set(on) {
      if (typeof on !== "boolean") fail("macroMode.set", `expected true or false, got ${show(on)}`);
      call("macroMode.set", { on });
    },
    toggle() {
      return call("macroMode.toggle") === true;
    },
  });

  const overlay = Object.freeze({
    show() {
      call("overlay.set", { value: "show" });
    },
    hide() {
      call("overlay.set", { value: "hide" });
    },
    toggle() {
      call("overlay.set", { value: "toggle" });
    },
    peek(ms) {
      const args = { value: "peek" };
      if (ms !== undefined) args.durationMs = millis("overlay.peek", "duration", ms);
      call("overlay.set", args);
    },
  });

  function toast(message, opts) {
    const o = options("toast", opts);
    call("toast", {
      text: text(message),
      durationMs: millis("toast", "durationMs", o.durationMs, 1600),
      kind: oneOf("toast", "kind", o.kind ?? "info", ["info", "success", "error"]),
    });
  }

  function notify(title, body) {
    call("notify", { title: text(title), body: body === undefined ? "" : text(body) });
  }

  const vars = Object.freeze({
    get(name, fallback = null) {
      return call("vars.get", { name: string("vars.get", "a variable name", name) }) ?? fallback;
    },
    set(name, value) {
      string("vars.set", "a variable name", name);
      if (typeof value === "function" || typeof value === "symbol") fail("vars.set", `values must be JSON data, got ${show(value)}`);
      call("vars.set", { name, value: value === undefined ? null : value });
    },
    toggle(name) {
      return call("vars.toggle", { name: string("vars.toggle", "a variable name", name) }) === true;
    },
  });

  const clipboard = Object.freeze({
    get() {
      const value = call("clipboard.get");
      return typeof value === "string" ? value : "";
    },
    set(value) {
      call("clipboard.set", { text: text(value) });
    },
  });

  const sound = Object.freeze({
    play(path) {
      call("sound.play", { path: string("sound.play", "a path", path) });
    },
  });

  const write = (level, args) => call("log", { level, message: args.map(text).join(" ") });

  function log(...args) {
    write("info", args);
  }

  const console = Object.freeze({
    log,
    info: log,
    debug: log,
    warn: (...args) => write("warn", args),
    error: (...args) => write("error", args),
  });

  const clockMinutes = (what, hhmm) => {
    const m = typeof hhmm === "string" ? /^\s*(\d{1,2}):(\d{2})\s*$/.exec(hhmm) : null;
    if (!m || Number(m[1]) > 23 || Number(m[2]) > 59) fail("isTimeBetween", `${what} must be a time like "09:30", got ${show(hhmm)}`);
    return Number(m[1]) * 60 + Number(m[2]);
  };

  // Local time in [from, to); wraps midnight when to < from; from === to means all day.
  function isTimeBetween(from, to) {
    const start = clockMinutes("from", from);
    const end = clockMinutes("to", to);
    const now = new Date();
    const minute = now.getHours() * 60 + now.getMinutes();
    if (start === end) return true;
    return start < end ? minute >= start && minute < end : minute >= start || minute < end;
  }

  // ---- install ------------------------------------------------------------------------------------------

  let currentEvent = null;
  Object.defineProperty(globalThis, "event", { get: () => currentEvent, enumerable: false, configurable: false });

  const api = {
    __host: host,
    __node: traceNode,
    sleep: nativeSleep,
    press,
    keyDown,
    keyUp,
    type,
    media,
    launch,
    closeApp,
    openUrl,
    openPath,
    run,
    window,
    desktop,
    kde,
    profile,
    macroMode,
    overlay,
    toast,
    notify,
    vars,
    clipboard,
    sound,
    log,
    console,
    isTimeBetween,
  };
  for (const [name, value] of Object.entries(api)) {
    Object.defineProperty(globalThis, name, { value, writable: false, enumerable: false, configurable: false });
  }

  const deepFreeze = (o) => {
    for (const v of Object.values(o)) if (typeof v === "object" && v !== null) deepFreeze(v);
    return Object.freeze(o);
  };

  // Private to the runtime (src/worker.rs); never reachable from user code.
  return Object.freeze({
    setEvent(json) {
      currentEvent = deepFreeze(JSON.parse(json));
    },
    // Runs the wrapped macro (src/source.rs) and returns its onRelease handler, if it defined one.
    runPress(main) {
      let findRelease = () => undefined;
      main((find) => {
        findRelease = find;
      });
      try {
        return findRelease();
      } catch {
        // `let onRelease = …` that an early `return` never reached.
        return undefined;
      }
    },
    runRelease(onRelease) {
      currentEvent = Object.freeze({ ...currentEvent, type: "release", repeat: false });
      onRelease(currentEvent);
    },
  });
};
