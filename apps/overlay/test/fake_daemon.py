#!/usr/bin/env python3
"""A stand-in for streampadd that serves the overlay side of the socket protocol (docs/SPEC.md §4).

    fake_daemon.py --socket /tmp/sp/daemon.sock --script "wait 1000; show; key KP5; next; toast hi; hide"
    fake_daemon.py --socket /tmp/sp/daemon.sock --interactive      # same commands, one per line on stdin

Script commands (separated by ';' or newlines):
    show | hide                    overlay.visible (pinned)
    output <connector|null>        overlay.output, e.g. "output DP-1"
    set <setting>=<json>           an overlay setting, e.g. "set edge=\"left\"" or "set scale=1.25"
    key <KeyId> [holdMs]           press, hold (default 140 ms) and release a key
    next | prev | profile <index>  switch profile (profile event, then state)
    toast [info|success|error:]<text>
    wait <ms>
    exec <shell command>           run a command and wait for it (e.g. a screenshot)
    quit                           stop serving and exit
"""

from __future__ import annotations

import argparse
import asyncio
import json
import os
import shlex
import signal
import sys
import threading
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fixtures as fx  # noqa: E402

START = time.monotonic()


def log(message: str) -> None:
    print(f"[fake-daemon {time.monotonic() - START:7.3f}s] {message}", flush=True)


class FakeDaemon:
    def __init__(self, socket_path: Path, settings: dict) -> None:
        self.socket_path = socket_path
        self.profiles = fx.with_stream(fx.PROFILES)
        self.resolve = fx.absolute_resolver(fx.find_share_dir())
        self.settings = settings
        self.index = 0
        self.visible = False
        self.output: str | None = None
        self.connections: set[asyncio.StreamWriter] = set()
        self.subscribers: set[asyncio.StreamWriter] = set()
        self.toast_id = 0
        self.stopped = asyncio.Event()

    def state(self) -> dict:
        return fx.daemon_state(self.profiles, self.index, settings=self.settings, visible=self.visible,
                               pinned=self.visible, output=self.output, resolve=self.resolve)

    # -- protocol --------------------------------------------------------------------------------

    async def serve(self) -> None:
        if self.socket_path.exists():
            self.socket_path.unlink()
        self.socket_path.parent.mkdir(parents=True, exist_ok=True)
        server = await asyncio.start_unix_server(self.handle_client, path=str(self.socket_path))
        os.chmod(self.socket_path, 0o600)
        log(f"listening on {self.socket_path}")
        async with server:
            await self.stopped.wait()
            # Drop every connection first: the server only finishes closing once they are gone.
            for writer in list(self.connections):
                writer.close()
        if self.socket_path.exists():
            self.socket_path.unlink()
        log("stopped")

    async def handle_client(self, reader: asyncio.StreamReader, writer: asyncio.StreamWriter) -> None:
        log("client connected")
        self.connections.add(writer)
        try:
            while line := await reader.readline():
                try:
                    request = json.loads(line)
                except json.JSONDecodeError:
                    log(f"ignoring malformed line: {line[:80]!r}")
                    continue
                result = self.dispatch(request.get("method"), request.get("params"), writer)
                self.send(writer, {"id": request.get("id"), "result": result})
        except (ConnectionResetError, BrokenPipeError):
            pass
        finally:
            self.connections.discard(writer)
            self.subscribers.discard(writer)
            writer.close()
            log("client disconnected")

    def dispatch(self, method: str, params, writer: asyncio.StreamWriter):
        if method == "hello":
            log(f"hello {json.dumps(params)}")
            if params and params.get("subscribe"):
                self.subscribers.add(writer)
            return {"version": "0.1.0-fake", "state": self.state()}
        if method == "getState":
            return self.state()
        if method == "reportOutputs":
            outputs = ", ".join(f"{o['name']}{' (primary)' if o.get('primary') else ''} "
                                f"{o['width']}x{o['height']}+{o['x']}+{o['y']} @{o['scale']}"
                                for o in params or [])
            log(f"reportOutputs: {outputs}")
            return {}
        log(f"unhandled method {method}")
        return {}

    def send(self, writer: asyncio.StreamWriter, message: dict) -> None:
        if not writer.is_closing():
            writer.write((json.dumps(message, ensure_ascii=False) + "\n").encode())

    def broadcast(self, event: str, data: dict) -> None:
        for writer in list(self.subscribers):
            self.send(writer, {"event": event, "data": data})

    def push_state(self) -> None:
        self.broadcast("state", self.state())

    # -- script ----------------------------------------------------------------------------------

    async def run(self, command: str) -> None:
        command = command.strip()
        if not command:
            return
        verb, _, rest = command.partition(" ")
        rest = rest.strip()
        log(f"> {command}")
        if verb in ("show", "hide"):
            self.visible = verb == "show"
            self.push_state()
        elif verb == "output":
            self.output = None if rest in ("", "null", "none") else rest
            self.push_state()
        elif verb == "set":
            name, _, value = rest.partition("=")
            self.settings[name.strip()] = json.loads(value)
            self.push_state()
        elif verb == "key":
            key, *hold = rest.split()
            kind = self.state()["profile"]["keys"][key]["kind"]
            self.broadcast("key", {"key": key, "pressed": True, "kind": kind})
            await asyncio.sleep(int(hold[0]) / 1000 if hold else 0.14)
            self.broadcast("key", {"key": key, "pressed": False, "kind": kind})
        elif verb in ("next", "prev", "profile"):
            count = len(self.profiles)
            direction = -1 if verb == "prev" else 1
            if verb == "profile":
                target = int(rest) % count
                direction = 1 if target >= self.index else -1
            else:
                target = (self.index + direction) % count
            self.index = target
            self.broadcast("profile", {"index": target, "direction": direction})
            self.push_state()
        elif verb == "toast":
            kind, sep, text = rest.partition(":")
            if not sep or kind not in ("info", "success", "error"):
                kind, text = "info", rest
            self.toast_id += 1
            self.broadcast("toast", {"id": self.toast_id, "text": text, "kind": kind, "durationMs": 2200})
        elif verb == "wait":
            await asyncio.sleep(int(rest) / 1000)
        elif verb == "exec":
            process = await asyncio.create_subprocess_exec(*shlex.split(rest))
            await process.wait()
        elif verb == "quit":
            self.stopped.set()
        else:
            log(f"unknown command: {command}")

    async def run_script(self, script: str) -> None:
        for command in script.replace("\n", ";").split(";"):
            if self.stopped.is_set():
                return
            await self.run(command)

    async def run_interactive(self) -> None:
        # A daemon thread reads stdin so a blocked readline never keeps the process alive on quit.
        loop = asyncio.get_running_loop()
        lines: asyncio.Queue[str] = asyncio.Queue()

        def read_stdin() -> None:
            for line in sys.stdin:
                loop.call_soon_threadsafe(lines.put_nowait, line)
            loop.call_soon_threadsafe(lines.put_nowait, "quit")

        threading.Thread(target=read_stdin, daemon=True).start()
        while not self.stopped.is_set():
            await self.run(await lines.get())


async def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--socket", default=os.environ.get("STREAMPAD_SOCKET"), help="socket path to serve")
    parser.add_argument("--script", help="commands to run once a client may have connected")
    parser.add_argument("--interactive", action="store_true", help="read commands from stdin")
    parser.add_argument("--set", action="append", default=[], metavar="NAME=JSON", help="initial overlay setting")
    args = parser.parse_args()
    if not args.socket:
        parser.error("--socket (or STREAMPAD_SOCKET) is required")

    settings = fx.default_settings()
    for item in args.set:
        name, _, value = item.partition("=")
        settings[name] = json.loads(value)

    daemon = FakeDaemon(Path(args.socket), settings)
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, daemon.stopped.set)

    tasks = [asyncio.create_task(daemon.serve())]
    if args.script:
        tasks.append(asyncio.create_task(daemon.run_script(args.script)))
    if args.interactive:
        tasks.append(asyncio.create_task(daemon.run_interactive()))
    await tasks[0]
    for task in tasks[1:]:
        task.cancel()


if __name__ == "__main__":
    asyncio.run(main())
