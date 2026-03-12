"""ntfy.sh notification outbox — enqueue to SQLite, send via background worker."""

from __future__ import annotations

import hashlib
import logging
from typing import TYPE_CHECKING

import httpx

if TYPE_CHECKING:
    from .config import OrchestrateConfig
    from .state import StateDB

log = logging.getLogger(__name__)

PRIORITY_MAP = {
    "pass": "default", "partial": "high", "blocked": "urgent",
    "failed": "urgent", "timeout": "high", "stall": "high", "error": "urgent",
}


async def _send_ntfy(
    topic: str,
    message: str,
    title: str = "",
    priority: str = "default",
    tags: str = "",
    click_url: str = "",
) -> bool:
    """POST a message to ntfy.sh. Returns True on HTTP 200."""
    headers: dict[str, str] = {"Priority": priority}
    if title:
        headers["Title"] = title
    if tags:
        headers["Tags"] = tags
    if click_url:
        headers["Click"] = click_url
    try:
        async with httpx.AsyncClient(timeout=10) as client:
            r = await client.post(
                f"https://ntfy.sh/{topic}",
                data=message.encode(),
                headers=headers,
            )
            return r.status_code == 200
    except Exception:
        return False


class Notifier:
    """Enqueues notifications to the SQLite outbox; a background worker delivers them."""

    def __init__(self, config: "OrchestrateConfig", state: "StateDB"):
        self._enabled = config.notify_enabled and bool(config.ntfy_topic)
        self._topic = config.ntfy_topic
        self._verbosity = config.notify_verbosity
        self._max_attempts = config.notify_max_attempts
        self._retry_delays = config.notify_retry_delays
        self._click_url = (
            f"http://localhost:{config.monitor_port}" if config.monitor_port else ""
        )
        self._state = state
        self._config = config

    def _should_send(self, kind: str) -> bool:
        v = self._verbosity
        if v == "all":
            return True
        if v == "failures_only":
            return kind in (
                "segment_complete_fail", "segment_stall", "gate_fail", "error", "finished"
            )
        if v == "waves_only":
            return kind in ("wave_complete", "gate_result", "finished", "error")
        if v == "final_only":
            return kind in ("finished", "error")
        return True

    async def enqueue(
        self,
        kind: str,
        message: str,
        title: str = "",
        priority: str = "default",
        tags: str = "",
    ) -> None:
        if not self._enabled or not self._should_send(kind):
            return
        event_key = hashlib.sha256(
            f"{kind}:{message[:200]}".encode()
        ).hexdigest()[:32]
        await self._state.enqueue_notification(kind, message, event_key, priority)

    async def started(self, plan_title: str, total: int, waves: int) -> None:
        await self.enqueue(
            "started",
            f"STARTED: {plan_title}\nSegments: {total} | Waves: {waves} | Parallel: {self._config.max_parallel}",
            title=f"Orchestration: {plan_title}",
            priority="default",
            tags="rocket",
        )

    async def wave_complete(
        self, wave: int, total_waves: int, results: list[tuple[int, str]]
    ) -> None:
        passed = sum(1 for _, s in results if s == "pass")
        failed = [(n, s) for n, s in results if s != "pass"]
        status = "PASS" if not failed else "FAIL"
        lines = [f"WAVE {wave}/{total_waves} {status}: {passed}/{len(results)} passed"]
        if failed:
            lines.append("Failed segments:")
            for n, s in failed:
                lines.append(f"  S{n:02d}: {s.upper()}")
        priority = "urgent" if failed else "default"
        await self.enqueue(
            "wave_complete",
            "\n".join(lines),
            title=f"Wave {wave}/{total_waves} {status}",
            priority=priority,
            tags="x" if failed else "white_check_mark",
        )

    async def segment_complete(
        self, num: int, title: str, status: str, summary: str
    ) -> None:
        kind = (
            "segment_complete_fail" if status not in ("pass",) else "segment_complete_pass"
        )
        # Compact format: status prefix + segment info + summary
        msg_lines = [
            f"S{num:02d} {status.upper()}: {title}",
            f"---",
            summary[:300] if summary else "(no summary)"
        ]
        await self.enqueue(
            kind,
            "\n".join(msg_lines),
            title=f"S{num:02d} {status.upper()}: {title[:30]}",
            priority=PRIORITY_MAP.get(status, "default"),
        )

    async def gate_result(self, wave: int, passed: bool, detail: str) -> None:
        kind = "gate_result" if passed else "gate_fail"
        status = "PASS" if passed else "FAIL"
        msg = f"GATE Wave {wave}: {status}"
        if not passed:
            msg += f"\n---\n{detail[:300]}"
        await self.enqueue(
            kind,
            msg,
            title=f"Gate Wave {wave}: {status}",
            priority="urgent" if not passed else "low",
        )

    async def stall(self, seg_num: int, minutes: int, activity: str) -> None:
        await self.enqueue(
            "segment_stall",
            f"STALL: S{seg_num:02d} ({minutes}min no output)\nLast activity:\n{activity[:200]}",
            title=f"S{seg_num:02d} STALLED ({minutes}min)",
            priority="high",
            tags="warning",
        )

    async def network_down(self, waited_sec: int) -> None:
        await self.enqueue(
            "network_down",
            f"NETWORK DOWN: Unreachable for {waited_sec}s\nOrchestration paused, waiting for connectivity",
            title=f"Network outage ({waited_sec}s)",
            priority="high",
            tags="satellite",
        )

    async def finished(self, plan_title: str, progress: dict[str, int]) -> None:
        total = sum(progress.values())
        passed = progress.get("pass", 0)
        status = "SUCCESS" if passed == total else "PARTIAL"

        # Build detailed status breakdown
        lines = [
            f"COMPLETE: {plan_title}",
            f"Status: {status} ({passed}/{total} passed)",
            "---",
            "Breakdown:"
        ]
        for status_type, count in sorted(progress.items()):
            if count > 0:
                lines.append(f"  {status_type}: {count}")

        await self.enqueue(
            "finished",
            "\n".join(lines),
            title=f"{status}: {plan_title}",
            tags="checkered_flag",
        )

    async def error(self, message: str) -> None:
        await self.enqueue(
            "error",
            f"ERROR: {message}",
            title="Orchestrator ERROR",
            priority="urgent",
            tags="fire",
        )

    async def heartbeat(self, summary: str) -> None:
        await self.enqueue(
            "heartbeat",
            f"STATUS: {summary}",
            title="Progress Update",
            priority="min",
        )
