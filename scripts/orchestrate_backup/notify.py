"""iMessage notifier using macOS osascript."""

from __future__ import annotations

import asyncio
import logging
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .config import OrchestrateConfig

log = logging.getLogger(__name__)


async def _send_imessage(contact: str, message: str) -> bool:
    """Send an iMessage via osascript. Returns True on success."""
    script = (
        f'tell application "Messages"\n'
        f'  set targetService to 1st account whose service type = iMessage\n'
        f'  set targetBuddy to participant "{contact}" of targetService\n'
        f'  send "{_escape_applescript(message)}" to targetBuddy\n'
        f'end tell'
    )
    try:
        proc = await asyncio.create_subprocess_exec(
            "osascript", "-e", script,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        _, stderr = await asyncio.wait_for(proc.communicate(), timeout=15)
        if proc.returncode != 0:
            log.warning("osascript failed: %s", stderr.decode().strip())
            return False
        return True
    except asyncio.TimeoutError:
        log.warning("osascript timed out sending to %s", contact)
        return False
    except Exception:
        log.exception("Failed to send iMessage")
        return False


def _escape_applescript(text: str) -> str:
    return text.replace("\\", "\\\\").replace('"', '\\"')


class Notifier:
    """Sends iMessage notifications for orchestration events."""

    def __init__(self, config: OrchestrateConfig):
        self._enabled = config.notify_enabled and bool(config.notify_contact)
        self._contact = config.notify_contact

    async def send(self, message: str) -> None:
        if not self._enabled:
            return
        await _send_imessage(self._contact, message)

    async def started(self, plan_title: str, total_segments: int, total_waves: int) -> None:
        await self.send(
            f"START Orchestration started\n"
            f"Plan: {plan_title}\n"
            f"{total_segments} segments in {total_waves} waves"
        )

    async def wave_start(self, wave: int, total_waves: int, segment_nums: list[int]) -> None:
        await self.send(
            f"📦 Wave {wave}/{total_waves} starting\n"
            f"Segments: {', '.join(f'S{n:02d}' for n in segment_nums)}"
        )

    async def segment_complete(
        self, num: int, title: str, status: str, summary: str
    ) -> None:
        icon = {"pass": "PASS", "partial": "[!]", "blocked": "FAIL", "failed": "FAIL"}.get(
            status.lower(), "UNKNOWN"
        )
        await self.send(
            f"{icon} S{num:02d} {status.upper()}: {title}\n{summary}"
        )

    async def gate_result(self, wave: int, passed: bool, detail: str) -> None:
        icon = "PASS" if passed else "BLOCK"
        status = "PASSED" if passed else "FAILED"
        msg = f"{icon} Gate after Wave {wave}: {status}"
        if not passed:
            # Truncate gate output to keep the message reasonable
            msg += f"\n{detail[:300]}"
        await self.send(msg)

    async def heartbeat(self, summary: str) -> None:
        await self.send(f"BEAT Heartbeat\n{summary}")

    async def finished(self, plan_title: str, progress: dict[str, int]) -> None:
        total = sum(progress.values())
        passed = progress.get("pass", 0)
        await self.send(
            f"🏁 Orchestration complete\n"
            f"Plan: {plan_title}\n"
            f"Results: {passed}/{total} passed\n"
            f"Breakdown: {progress}"
        )

    async def error(self, message: str) -> None:
        await self.send(f"ERROR Orchestrator Error\n{message}")
