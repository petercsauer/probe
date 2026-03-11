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

from .streamparse import _extract_text_from_stream_line

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

    # CRITICAL FIX: Remove CLAUDECODE to prevent nested session detection
    env.pop('CLAUDECODE', None)
    env.pop('CLAUDE_CODE_ENTRYPOINT', None)

    env.update(config.auth_env)
    env.update(_resolve_isolation_env(seg_num, config))
    return env


async def _kill_tree(pid: int, grace_seconds: int = 5) -> bool:
    """Kill entire process group rooted at pid.

    Returns True if successful, False otherwise.
    """
    try:
        pgid = os.getpgid(pid)
        os.killpg(pgid, signal.SIGTERM)
        await asyncio.sleep(grace_seconds)
        try:
            os.killpg(pgid, signal.SIGKILL)
        except ProcessLookupError:
            log.info("Process %d already terminated", pid)
        return True
    except ProcessLookupError:
        log.info("Process %d already terminated", pid)
        return True
    except PermissionError:
        log.warning("Cannot kill process %d: permission denied", pid)
        return False
    except Exception as e:
        log.error("Failed to kill process %d: %s", pid, e)
        return False


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


async def _segment_heartbeat_task(
    seg_num: int,
    raw_log: Path,
    state: "StateDB",
    notifier,
    started_at: float,
    heartbeat_interval: int = 60,
    stall_threshold: int = 1800,
) -> None:
    """Write last_seen_at/last_activity every heartbeat_interval seconds.

    Detects stalls (file size unchanged past stall_threshold) and enqueues a
    notification. All errors are swallowed so the heartbeat never kills the run.
    """
    last_size, stall_notified = 0, False
    while True:
        await asyncio.sleep(heartbeat_interval)
        activity, current_size = "", 0
        try:
            if raw_log.exists():
                raw = raw_log.read_bytes()
                current_size = len(raw)
                # Discard potentially incomplete last line with [:-1]
                tail = raw[-2048:].decode("utf-8", errors="replace")
                for line in reversed(tail.splitlines()[:-1]):
                    text = _extract_text_from_stream_line(line)
                    if text and text.strip():
                        activity = text.strip()[:500]
                        break
        except Exception:
            pass
        try:
            await state.update_heartbeat(seg_num, time.time(), activity)
        except Exception:
            pass
        elapsed = time.time() - started_at
        if elapsed > stall_threshold and current_size == last_size:
            if not stall_notified and notifier:
                try:
                    await notifier.stall(seg_num, stall_threshold // 60, activity)
                except Exception:
                    pass
                stall_notified = True
        else:
            stall_notified = False
        last_size = current_size


def _extract_token_usage(raw_path: Path) -> tuple[int, int]:
    """Parse token counts from the stream-json result event.

    Returns (input_tokens, output_tokens), or (0, 0) on any failure.
    """
    try:
        with open(raw_path, encoding="utf-8", errors="replace") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    obj = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if obj.get("type") == "result":
                    u = obj.get("usage", {})
                    return u.get("input_tokens", 0), u.get("output_tokens", 0)
    except Exception:
        pass
    return 0, 0


async def run_segment(
    seg: "Segment",
    config: "OrchestrateConfig",
    state: "StateDB",
    log_dir: Path,
    notifier=None,
    attempt_num: int = 1,
    register_pid=None,
    unregister_pid=None,
    cwd: Path | None = None,
) -> tuple[str, str]:
    """Execute a single segment via claude CLI.

    Args:
        cwd: Optional working directory for the subprocess. When isolation_strategy="worktree",
             the orchestrator will pass the worktree path here.

    Returns (status, summary).
    """
    prompt = _build_prompt(seg, config)
    env = _build_env(seg.num, config)
    raw_log = log_dir / f"S{seg.num:02d}.stream.jsonl"
    human_log = log_dir / f"S{seg.num:02d}.log"
    prompt_file = log_dir / f"S{seg.num:02d}.prompt.txt"

    prompt_file.write_text(prompt, encoding="utf-8")
    # Clear stale files from prior runs. Do NOT create human_log yet — its
    # presence signals to the SSE handler that the segment is finished, which
    # would prevent real-time stream.jsonl content from being shown.
    raw_log.write_text("", encoding="utf-8")
    human_log.unlink(missing_ok=True)

    started_at = time.time()
    await state.set_status(seg.num, "running", started_at=started_at)
    await state.log_event("segment_start", f"S{seg.num:02d} {seg.title}")

    log.info("S%02d starting: %s", seg.num, seg.title)

    # Per-segment timeout: frontmatter `timeout` field overrides global default.
    segment_timeout = getattr(seg, "timeout", 0) or config.segment_timeout
    stall_threshold = getattr(config, "stall_threshold", 1800)

    heartbeat: asyncio.Task | None = None

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
            cwd=cwd,  # Use provided working directory (worktree path when isolation_strategy="worktree")
            start_new_session=True,
            limit=2**22,  # 4MB — default 64KB trips on large tool-result JSON lines
        )

        if register_pid and proc.pid:
            register_pid(seg.num, proc.pid)

        heartbeat = asyncio.create_task(
            _segment_heartbeat_task(
                seg.num, raw_log, state, notifier, started_at,
                stall_threshold=stall_threshold,
            )
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
            await asyncio.wait_for(_drain_stdout(), timeout=segment_timeout)
        except asyncio.TimeoutError:
            log.warning("S%02d timed out after %ds", seg.num, segment_timeout)
            if proc.pid:
                await _kill_tree(proc.pid)
            finished_at = time.time()
            await state.set_status(seg.num, "timeout", finished_at=finished_at)
            await state.log_event(
                "segment_timeout", f"S{seg.num:02d} killed after {segment_timeout}s"
            )
            tokens_in, tokens_out = _extract_token_usage(raw_log)
            await state.record_attempt(
                seg.num, attempt_num, started_at, finished_at,
                "timeout", f"Killed after {segment_timeout}s",
                tokens_in, tokens_out,
            )
            return "timeout", f"Killed after {segment_timeout}s"

        await proc.wait()

    except Exception as exc:
        log.exception("S%02d process error", seg.num)
        finished_at = time.time()
        await state.set_status(seg.num, "failed", finished_at=finished_at)
        await state.log_event("segment_error", f"S{seg.num:02d}: {exc}")
        await state.record_attempt(
            seg.num, attempt_num, started_at, finished_at,
            "failed", str(exc), 0, 0,
        )
        return "failed", str(exc)

    finally:
        try:
            if unregister_pid:
                unregister_pid(seg.num)
        except Exception:
            log.exception("Failed to unregister PID for S%02d", seg.num)
        finally:
            if heartbeat is not None:
                heartbeat.cancel()
                try:
                    await heartbeat
                except asyncio.CancelledError:
                    pass

    # Parse the stream output
    full_text, _result = _parse_stream_jsonl(raw_log)
    human_log.write_text(full_text, encoding="utf-8")

    status = _extract_status(full_text)
    summary = _extract_summary(full_text)
    finished_at = time.time()

    await state.set_status(
        seg.num, status,
        finished_at=finished_at,
        result_json=json.dumps({"status": status, "summary": summary}),
    )
    await state.log_event(
        "segment_complete",
        f"S{seg.num:02d} {status.upper()}: {summary[:200]}",
    )

    tokens_in, tokens_out = _extract_token_usage(raw_log)
    await state.record_attempt(
        seg.num, attempt_num, started_at, finished_at,
        status, summary, tokens_in, tokens_out,
    )

    log.info("S%02d finished: %s", seg.num, status.upper())
    return status, summary
