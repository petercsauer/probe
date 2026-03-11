"""Segment runner: prompt assembly, claude execution, log writing, timeout."""

from __future__ import annotations

import asyncio
import json
import logging
import os
import signal
import time
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .config import OrchestrateConfig
    from .planner import Segment
    from .state import StateDB

log = logging.getLogger(__name__)


def _resolve_isolation_env(seg_num: int, config: OrchestrateConfig) -> dict[str, str]:
    """Expand isolation env var templates for a specific segment."""
    env: dict[str, str] = {}
    if config.isolation_strategy != "env":
        return env
    for key, template in config.isolation_env.items():
        try:
            env[key] = template.format(num=seg_num)
        except (KeyError, IndexError, ValueError):
            env[key] = template.replace("{num}", str(seg_num))
    return env


def _build_prompt(seg: "Segment", config: OrchestrateConfig) -> str:
    """Assemble the prompt for a segment's claude session."""
    parts: list[str] = []
    parts.append("You are an iterative-builder. Build ONE segment and report results.\n")

    for preamble_path in config.preamble_files:
        parts.append(f"Read and follow `{preamble_path}` exactly.\n")

    parts.append(f"Read `{seg.file_path}` — this is what you must build.\n")

    if config.extra_rules:
        iso_env = _resolve_isolation_env(seg.num, config)
        iso_dir = next(iter(iso_env.values()), "N/A") if iso_env else "N/A"
        try:
            rules = config.extra_rules.format(
                segment_num=seg.num,
                segment_title=seg.title,
                isolation_dir=iso_dir,
            )
        except (KeyError, IndexError):
            rules = config.extra_rules
        parts.append(rules.strip() + "\n")

    parts.append("Begin now.")
    return "\n".join(parts)


def _build_env(seg_num: int, config: OrchestrateConfig) -> dict[str, str]:
    """Build environment dict: inherit shell + auth + isolation."""
    env = dict(os.environ)
    env.update(config.auth_env)
    env.update(_resolve_isolation_env(seg_num, config))
    return env


async def _kill_tree(pid: int, grace_seconds: int = 5) -> None:
    """Kill entire process group rooted at pid."""
    try:
        pgid = os.getpgid(pid)
        os.killpg(pgid, signal.SIGTERM)
        await asyncio.sleep(grace_seconds)
        try:
            os.killpg(pgid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    except (ProcessLookupError, PermissionError):
        pass


def _parse_stream_jsonl(raw_path: Path) -> tuple[str, str]:
    """Parse a claude --output-format stream-json file.

    Returns (human_readable_log, final_result_text).
    """
    lines: list[str] = []
    result_text = ""
    try:
        with open(raw_path, encoding="utf-8", errors="replace") as f:
            for raw_line in f:
                raw_line = raw_line.strip()
                if not raw_line:
                    continue
                try:
                    msg = json.loads(raw_line)
                except json.JSONDecodeError:
                    lines.append(raw_line)
                    continue

                msg_type = msg.get("type", "")

                if msg_type == "assistant" and "message" in msg:
                    # Assistant text block
                    content = msg["message"].get("content", [])
                    for block in content:
                        if isinstance(block, dict) and block.get("type") == "text":
                            lines.append(block["text"])
                elif msg_type == "content_block_delta":
                    delta = msg.get("delta", {})
                    if delta.get("type") == "text_delta":
                        lines.append(delta.get("text", ""))
                elif msg_type == "result":
                    result_text = msg.get("result", "")
                    if not result_text:
                        sub = msg.get("subresult", "")
                        if sub:
                            result_text = sub
    except FileNotFoundError:
        pass
    return "".join(lines), result_text


def _extract_status(log_text: str) -> str:
    """Extract PASS/PARTIAL/BLOCKED from the builder report in log text."""
    for marker in ("**Status:** PASS", "Status: PASS"):
        if marker in log_text:
            return "pass"
    for marker in ("**Status:** PARTIAL", "Status: PARTIAL"):
        if marker in log_text:
            return "partial"
    for marker in ("**Status:** BLOCKED", "Status: BLOCKED"):
        if marker in log_text:
            return "blocked"
    return "unknown"


def _extract_summary(log_text: str, max_len: int = 300) -> str:
    """Extract a brief summary from the builder report."""
    for header in ("### What was built", "## What was built"):
        idx = log_text.find(header)
        if idx >= 0:
            chunk = log_text[idx + len(header):idx + len(header) + max_len]
            return chunk.strip().split("\n\n")[0].strip()
    # Fallback: last 200 chars
    return log_text[-200:].strip() if log_text else "(no output)"


async def run_segment(
    seg: "Segment",
    config: OrchestrateConfig,
    state: "StateDB",
    log_dir: Path,
) -> tuple[str, str]:
    """Execute a single segment via claude CLI.

    Returns (status, summary).
    """
    prompt = _build_prompt(seg, config)
    env = _build_env(seg.num, config)
    raw_log = log_dir / f"S{seg.num:02d}.stream.jsonl"
    human_log = log_dir / f"S{seg.num:02d}.log"
    prompt_file = log_dir / f"S{seg.num:02d}.prompt.txt"

    prompt_file.write_text(prompt, encoding="utf-8")
    # Clear stale log files from prior runs so the dashboard never shows old content.
    raw_log.write_text("", encoding="utf-8")
    human_log.write_text("(running…)\n", encoding="utf-8")

    state.set_status(seg.num, "running", started_at=time.time())
    state.log_event("segment_start", f"S{seg.num:02d} {seg.title}")

    log.info("S%02d starting: %s", seg.num, seg.title)

    try:
        proc = await asyncio.create_subprocess_exec(
            "claude",
            "-p", prompt,
            "--dangerously-skip-permissions",
            "--verbose",
            "--output-format", "stream-json",
            stdin=asyncio.subprocess.DEVNULL,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.STDOUT,
            env=env,
            start_new_session=True,
        )

        async def _drain_stdout():
            with open(raw_log, "w", encoding="utf-8") as f:
                while True:
                    line = await proc.stdout.readline()
                    if not line:
                        break
                    f.write(line.decode("utf-8", errors="replace"))
                    f.flush()

        try:
            await asyncio.wait_for(_drain_stdout(), timeout=config.segment_timeout)
        except asyncio.TimeoutError:
            log.warning("S%02d timed out after %ds", seg.num, config.segment_timeout)
            if proc.pid:
                await _kill_tree(proc.pid)
            state.set_status(seg.num, "timeout", finished_at=time.time())
            state.log_event("segment_timeout", f"S{seg.num:02d} killed after {config.segment_timeout}s")
            return "timeout", f"Killed after {config.segment_timeout}s"

        await proc.wait()

    except Exception as exc:
        log.exception("S%02d process error", seg.num)
        state.set_status(seg.num, "failed", finished_at=time.time())
        state.log_event("segment_error", f"S{seg.num:02d}: {exc}")
        return "failed", str(exc)

    # Parse the stream output
    full_text, _result = _parse_stream_jsonl(raw_log)
    human_log.write_text(full_text, encoding="utf-8")

    status = _extract_status(full_text)
    summary = _extract_summary(full_text)

    state.set_status(
        seg.num, status,
        finished_at=time.time(),
        result_json=json.dumps({"status": status, "summary": summary}),
    )
    state.log_event(
        "segment_complete",
        f"S{seg.num:02d} {status.upper()}: {summary[:200]}",
    )

    log.info("S%02d finished: %s", seg.num, status.upper())
    return status, summary
