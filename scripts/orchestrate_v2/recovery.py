"""Recovery agent: workspace health checking and cascade victim detection."""

from __future__ import annotations

import asyncio
import json
import logging
from dataclasses import dataclass
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .config import OrchestrateConfig
    from .state import StateDB

log = logging.getLogger(__name__)


@dataclass
class RecoveryConfig:
    """Configuration for the recovery agent."""

    enabled: bool = True
    health_check_timeout: int = 300  # 5 minutes
    victim_markers: list[str] = None

    def __post_init__(self):
        if self.victim_markers is None:
            self.victim_markers = [
                "pre-existing errors",
                "my code is correct",
                "blocked by S",
            ]


class RecoveryAgent:
    """Recovery agent for post-wave workspace health checks and victim detection."""

    def __init__(self, state: "StateDB", config: "OrchestrateConfig"):
        """Initialize recovery agent with state DB and orchestration config.

        Args:
            state: StateDB instance for querying segment statuses
            config: OrchestrateConfig instance for workspace and execution settings
        """
        self.state = state
        self.config = config
        self.recovery_config = RecoveryConfig()

    async def check_workspace_health(self) -> tuple[bool, list[str]]:
        """Check if workspace builds cleanly using cargo check.

        Returns:
            tuple: (healthy: bool, error_list: list[str])
                - healthy: True if no compilation errors, False otherwise
                - error_list: List of error messages from cargo check
        """
        log.info("Running workspace health check...")

        try:
            proc = await asyncio.create_subprocess_exec(
                "cargo",
                "check",
                "--workspace",
                "--message-format=json",
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE,
                limit=2**22,  # 4MB buffer like in runner.py
            )

            # Wait with timeout
            try:
                stdout, stderr = await asyncio.wait_for(
                    proc.communicate(),
                    timeout=self.recovery_config.health_check_timeout,
                )
            except asyncio.TimeoutError:
                log.error(
                    "Workspace health check timed out after %ds",
                    self.recovery_config.health_check_timeout,
                )
                try:
                    if hasattr(proc, "kill"):
                        proc.kill()
                    if hasattr(proc, "wait"):
                        await proc.wait()
                except Exception:
                    pass
                return False, [
                    f"Health check timeout after {self.recovery_config.health_check_timeout}s"
                ]

            # Parse JSON output for errors
            errors = []
            stdout_text = stdout.decode("utf-8", errors="replace")

            for line in stdout_text.splitlines():
                line = line.strip()
                if not line:
                    continue

                try:
                    msg = json.loads(line)
                except json.JSONDecodeError:
                    continue

                # Look for compiler error messages
                if msg.get("reason") == "compiler-message":
                    message = msg.get("message", {})
                    if message.get("level") == "error":
                        rendered = message.get("rendered", "")
                        if rendered:
                            errors.append(rendered.strip())

            # Check return code
            if proc.returncode != 0:
                if not errors:
                    # Non-zero exit but no parsed errors - include stderr
                    stderr_text = stderr.decode("utf-8", errors="replace")
                    if stderr_text.strip():
                        errors.append(stderr_text.strip())
                log.warning("Workspace health check failed with %d errors", len(errors))
                return False, errors

            log.info("Workspace health check passed")
            return True, []

        except Exception as exc:
            log.exception("Failed to run workspace health check")
            return False, [f"Health check exception: {exc}"]

    async def identify_cascade_victims(
        self, wave_segments: list
    ) -> list[int]:
        """Identify segments that failed due to pre-existing errors from prior segments.

        Reads builder reports from logs/ directory and looks for victim markers:
        - "pre-existing errors"
        - "my code is correct"
        - "blocked by S{N}"

        Args:
            wave_segments: List of SegmentRow objects from the completed wave

        Returns:
            List of segment numbers that should be retried
        """
        log.info("Scanning for cascade victims in wave...")

        victims = []

        for seg in wave_segments:
            # Only consider failed segments (blocked/partial)
            if seg.status not in ("blocked", "partial"):
                continue

            # Read the builder report log
            log_path = Path(f"logs/S{seg.num:02d}.log")
            if not log_path.exists():
                log.warning("Log file not found for S%02d", seg.num)
                continue

            try:
                log_text = log_path.read_text(encoding="utf-8", errors="replace")
            except Exception as exc:
                log.warning("Failed to read log for S%02d: %s", seg.num, exc)
                continue

            # Check for victim markers
            is_victim = False
            for marker in self.recovery_config.victim_markers:
                if marker.lower() in log_text.lower():
                    log.info(
                        "S%02d: Found victim marker '%s'",
                        seg.num,
                        marker,
                    )
                    is_victim = True
                    break

            if is_victim:
                victims.append(seg.num)

        if victims:
            log.info("Identified %d cascade victims: %s", len(victims), victims)
        else:
            log.info("No cascade victims identified")

        return victims
