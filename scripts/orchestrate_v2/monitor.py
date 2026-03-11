"""HTTP monitor server: dashboard, state API, log SSE stream."""

from __future__ import annotations

import asyncio
import json
import logging
import os
import signal as signal_mod
import time
from pathlib import Path
from typing import TYPE_CHECKING

import aiofiles
import aiofiles.os
from aiohttp import web

from .streamparse import _parse_stream_line_rich

if TYPE_CHECKING:
    from .state import StateDB

log = logging.getLogger(__name__)

_DASHBOARD_PATH = Path(__file__).parent / "dashboard.html"


def _create_app(state: "StateDB", log_dir: Path, running_pids: dict | None = None) -> web.Application:
    app = web.Application()
    app["state"] = state
    app["log_dir"] = log_dir
    app["running_pids"] = running_pids if running_pids is not None else {}
    app.router.add_get("/", _handle_dashboard)
    app.router.add_get("/api/state", _handle_state)
    app.router.add_get("/api/events", _handle_events_sse)
    app.router.add_get("/api/logs/{seg_id}", _handle_log_sse)
    app.router.add_post("/api/control", _handle_control)
    return app


async def _handle_dashboard(request: web.Request) -> web.Response:
    try:
        html = _DASHBOARD_PATH.read_text(encoding="utf-8")
    except FileNotFoundError:
        html = "<html><body><h1>Dashboard not found</h1></body></html>"
    return web.Response(text=html, content_type="text/html")


async def _handle_state(request: web.Request) -> web.Response:
    state: StateDB = request.app["state"]
    return web.json_response(await state.all_as_dict())


async def _handle_control(request: web.Request) -> web.Response:
    """POST /api/control — skip/retry/kill a segment by operator action."""
    state: StateDB = request.app["state"]
    pids: dict = request.app["running_pids"]
    try:
        data = await request.json()
    except Exception:
        return web.json_response({"ok": False, "error": "invalid JSON"}, status=400)

    action = data.get("action")
    try:
        seg_num = int(data.get("seg_num", 0))
    except (TypeError, ValueError):
        return web.json_response({"ok": False, "error": "invalid seg_num"}, status=400)

    if action == "skip":
        pid = pids.get(seg_num)
        if pid:
            try:
                os.killpg(os.getpgid(pid), signal_mod.SIGTERM)
            except Exception:
                pass
        await state.set_status(seg_num, "skipped")
        await state.log_event("operator_skip", f"S{seg_num:02d} skipped by operator", severity="warn")
        return web.json_response({"ok": True, "action": "skip", "seg_num": seg_num})

    elif action == "retry":
        await state.reset_for_retry(seg_num)
        await state.log_event(
            "operator_retry",
            f"S{seg_num:02d} reset for retry (restart orchestrator to run)",
            severity="warn",
        )
        return web.json_response({"ok": True, "action": "retry", "seg_num": seg_num})

    elif action == "kill":
        pid = pids.get(seg_num)
        if not pid:
            return web.json_response({"ok": False, "error": "not running"}, status=404)
        try:
            os.killpg(os.getpgid(pid), signal_mod.SIGTERM)
            await state.log_event("operator_kill", f"S{seg_num:02d} killed by operator", severity="warn")
            return web.json_response({"ok": True, "action": "kill", "seg_num": seg_num})
        except Exception as e:
            return web.json_response({"ok": False, "error": str(e)}, status=500)

    return web.json_response({"ok": False, "error": f"unknown action: {action}"}, status=400)


async def _handle_events_sse(request: web.Request) -> web.StreamResponse:
    """Stream events as SSE. Client connects to /api/events."""
    state: StateDB = request.app["state"]
    response = web.StreamResponse()
    response.content_type = "text/event-stream"
    response.headers["Cache-Control"] = "no-cache"
    response.headers["X-Accel-Buffering"] = "no"
    await response.prepare(request)

    last_id = 0
    try:
        while True:
            events = await state.get_events(limit=20, after_id=last_id)
            for ev in reversed(events):  # oldest first
                data = json.dumps(ev)
                await response.write(f"id: {ev['id']}\ndata: {data}\n\n".encode())
                last_id = max(last_id, ev["id"])
            await asyncio.sleep(2)
    except (asyncio.CancelledError, ConnectionResetError):
        pass
    return response




async def _handle_log_sse(request: web.Request) -> web.StreamResponse:
    """Stream a segment's log as SSE with rich structured events."""
    seg_id = request.match_info["seg_id"]
    log_dir: Path = request.app["log_dir"]
    log_file = log_dir / f"{seg_id}.log"
    stream_file = log_dir / f"{seg_id}.stream.jsonl"

    response = web.StreamResponse()
    response.content_type = "text/event-stream"
    response.headers["Cache-Control"] = "no-cache"
    response.headers["X-Accel-Buffering"] = "no"
    await response.prepare(request)

    async def _emit(event: dict) -> None:
        await response.write(f"data: {json.dumps(event)}\n\n".encode())

    byte_offset = 0
    using_log = False
    # Buffer incomplete lines across reads
    line_buf = ""
    try:
        while True:
            # Once the final .log exists, switch to it (segment finished)
            if not using_log and await aiofiles.os.path.exists(str(log_file)) and (await aiofiles.os.stat(str(log_file))).st_size > 0:
                using_log = True
                byte_offset = 0
                line_buf = ""
                # Emit a marker so the dashboard can show the completed log header
                await _emit({"type": "_switch_to_log"})

            target = log_file if using_log else stream_file
            if await aiofiles.os.path.exists(str(target)):
                async with aiofiles.open(target, 'rb') as f:
                    raw = await f.read()
                if len(raw) > byte_offset:
                    new_bytes = raw[byte_offset:]
                    byte_offset = len(raw)
                    new_text = line_buf + new_bytes.decode("utf-8", errors="replace")

                    if using_log:
                        # Finished log: plain text lines emitted as text events
                        lines = new_text.split("\n")
                        line_buf = lines[-1]  # keep incomplete last chunk
                        for line in lines[:-1]:
                            if line.strip():
                                await _emit({"type": "text", "text": line})
                    else:
                        # Live stream: parse stream-json for rich events
                        lines = new_text.split("\n")
                        line_buf = lines[-1]  # keep incomplete last chunk
                        for line in lines[:-1]:
                            for event in _parse_stream_line_rich(line):
                                await _emit(event)

            await asyncio.sleep(0.5)
    except (asyncio.CancelledError, ConnectionResetError):
        pass
    return response


class MonitorServer:
    """Manages the aiohttp dashboard server lifecycle."""

    def __init__(self, state: "StateDB", log_dir: Path, port: int, running_pids: dict | None = None):
        self._state = state
        self._log_dir = log_dir
        self._port = port
        self._running_pids = running_pids if running_pids is not None else {}
        self._runner: web.AppRunner | None = None

    async def start(self) -> None:
        if self._port <= 0:
            return
        app = _create_app(self._state, self._log_dir, self._running_pids)
        self._runner = web.AppRunner(app)
        await self._runner.setup()
        site = web.TCPSite(self._runner, "0.0.0.0", self._port, reuse_address=True)
        try:
            await site.start()
        except OSError as exc:
            log.warning("Monitor bind failed on port %d: %s (continuing without dashboard)", self._port, exc)
            self._port = 0
            return
        log.info("Monitor dashboard: http://localhost:%d", self._port)

    async def stop(self) -> None:
        if self._runner:
            await self._runner.cleanup()
