"""HTTP monitor server: dashboard, state API, log SSE stream."""

from __future__ import annotations

import asyncio
import json
import logging
import time
from pathlib import Path
from typing import TYPE_CHECKING

from aiohttp import web

if TYPE_CHECKING:
    from .state import StateDB

log = logging.getLogger(__name__)

_DASHBOARD_PATH = Path(__file__).parent / "dashboard.html"


def _create_app(state: "StateDB", log_dir: Path) -> web.Application:
    app = web.Application()
    app["state"] = state
    app["log_dir"] = log_dir
    app.router.add_get("/", _handle_dashboard)
    app.router.add_get("/api/state", _handle_state)
    app.router.add_get("/api/events", _handle_events_sse)
    app.router.add_get("/api/logs/{seg_id}", _handle_log_sse)
    return app


async def _handle_dashboard(request: web.Request) -> web.Response:
    try:
        html = _DASHBOARD_PATH.read_text(encoding="utf-8")
    except FileNotFoundError:
        html = "<html><body><h1>Dashboard not found</h1></body></html>"
    return web.Response(text=html, content_type="text/html")


async def _handle_state(request: web.Request) -> web.Response:
    state: StateDB = request.app["state"]
    return web.json_response(state.all_as_dict())


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
            events = state.get_events(limit=20, after_id=last_id)
            for ev in reversed(events):  # oldest first
                data = json.dumps(ev)
                await response.write(f"id: {ev['id']}\ndata: {data}\n\n".encode())
                last_id = max(last_id, ev["id"])
            await asyncio.sleep(2)
    except (asyncio.CancelledError, ConnectionResetError):
        pass
    return response


async def _handle_log_sse(request: web.Request) -> web.StreamResponse:
    """Stream a segment's log file as SSE. Client connects to /api/logs/S01."""
    seg_id = request.match_info["seg_id"]
    log_dir: Path = request.app["log_dir"]
    log_file = log_dir / f"{seg_id}.log"
    stream_file = log_dir / f"{seg_id}.stream.jsonl"

    response = web.StreamResponse()
    response.content_type = "text/event-stream"
    response.headers["Cache-Control"] = "no-cache"
    response.headers["X-Accel-Buffering"] = "no"
    await response.prepare(request)

    offset = 0
    try:
        while True:
            # Prefer the human-readable log, fall back to stream file
            target = log_file if log_file.exists() else stream_file
            if target.exists():
                content = target.read_text(encoding="utf-8", errors="replace")
                if len(content) > offset:
                    new_data = content[offset:]
                    offset = len(content)
                    for line in new_data.splitlines(keepends=True):
                        escaped = json.dumps(line.rstrip("\n"))
                        await response.write(f"data: {escaped}\n\n".encode())
            await asyncio.sleep(1)
    except (asyncio.CancelledError, ConnectionResetError):
        pass
    return response


class MonitorServer:
    """Manages the aiohttp dashboard server lifecycle."""

    def __init__(self, state: "StateDB", log_dir: Path, port: int):
        self._state = state
        self._log_dir = log_dir
        self._port = port
        self._runner: web.AppRunner | None = None

    async def start(self) -> None:
        if self._port <= 0:
            return
        app = _create_app(self._state, self._log_dir)
        self._runner = web.AppRunner(app)
        await self._runner.setup()
        site = web.TCPSite(self._runner, "0.0.0.0", self._port)
        await site.start()
        log.info("Monitor dashboard: http://localhost:%d", self._port)

    async def stop(self) -> None:
        if self._runner:
            await self._runner.cleanup()
