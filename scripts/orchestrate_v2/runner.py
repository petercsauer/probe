"""Segment runner: prompt assembly, claude execution, log writing, timeout."""

from __future__ import annotations

import asyncio
import json
import logging
import os
import re
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


class CircuitBreaker:
    """Detects permanent failures and prevents wasteful retries.

    Implements simplified circuit breaker pattern for batch orchestration.
    Checks error messages against known permanent failure patterns.
    """

    # Permanent failure patterns (regex for flexibility)
    PERMANENT_PATTERNS = {
        "nested_session": r"Claude Code cannot be launched inside another",
        "missing_file": r"No such file or directory",
        "permission_denied": r"Permission denied",
        "syntax_error": r"SyntaxError:|syntax error",
        "module_not_found": r"ModuleNotFoundError|No module named",
        "import_error": r"ImportError:|cannot import",
        "invalid_config": r"Invalid configuration|Config validation failed",
    }

    def should_retry(self, error_message: str) -> tuple[bool, str]:
        """Check if error is retryable based on pattern matching.

        Args:
            error_message: Error text from segment output

        Returns:
            (should_retry: bool, reason: str)

        Examples:
            >>> cb = CircuitBreaker()
            >>> cb.should_retry("Permission denied for /etc/shadow")
            (False, "Permanent failure pattern detected: permission_denied")
            >>> cb.should_retry("Connection timeout after 30s")
            (True, "")
        """
        for pattern_name, pattern in self.PERMANENT_PATTERNS.items():
            if re.search(pattern, error_message, re.IGNORECASE):
                reason = f"Permanent failure pattern detected: {pattern_name}"
                log.warning(f"Circuit breaker: {reason}")
                return False, reason

        # No permanent pattern detected - allow retry
        return True, ""

    def add_pattern(self, name: str, pattern: str):
        """Add custom permanent failure pattern (for extensibility)."""
        self.PERMANENT_PATTERNS[name] = pattern


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


def _build_prompt(
    seg: "Segment",
    config: OrchestrateConfig,
    interject: str | None = None,
    partial_context: str | None = None,
) -> str:
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

    if partial_context:
        parts.append("\n" + "="*60)
        parts.append("🔄 PARTIAL COMPLETION - CONTINUE FROM WHERE YOU LEFT OFF")
        parts.append("="*60)
        parts.append("\nThe previous attempt reached PARTIAL status (made progress but didn't complete).")
        parts.append("The code from that attempt is still in the worktree - DO NOT start from scratch.")
        parts.append("\nPrevious attempt summary:")
        parts.append(partial_context)
        parts.append("\nContinue from where the previous attempt left off. Review what was accomplished,")
        parts.append("identify remaining work, and complete the segment to reach PASS status.")
        parts.append("="*60 + "\n")

    if interject:
        parts.append("\n" + "="*60)
        parts.append("⚠️  OPERATOR INTERJECT MESSAGE")
        parts.append("="*60)
        parts.append(interject)
        parts.append("")
        parts.append("Address this feedback and continue the segment.")
        parts.append("="*60 + "\n")

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
    """Extract PASS/PARTIAL/BLOCKED from the builder report in log text.

    Uses exact string matching as the primary method (fast path), with regex
    fallback patterns for common variations. Returns the first match found.
    Logs warnings when non-standard formats are detected.
    """
    import re

    # Find the earliest occurrence among all exact markers
    earliest_pos = len(log_text)
    earliest_status = None

    # Check all exact markers and track the earliest
    markers = [
        ("**Status:** PASS", "pass"),
        ("Status: PASS", "pass"),
        ("**Status:** ✅ PASS", "pass"),  # With checkmark emoji
        ("Status: ✅ PASS", "pass"),
        ("**Status:** PARTIAL", "partial"),
        ("Status: PARTIAL", "partial"),
        ("**Status:** BLOCKED", "blocked"),
        ("Status: BLOCKED", "blocked"),
    ]

    for marker, status in markers:
        pos = log_text.find(marker)
        if pos != -1 and pos < earliest_pos:
            earliest_pos = pos
            earliest_status = status

    if earliest_status is not None:
        return earliest_status

    # Lenient patterns for common variations
    # Patterns accept both emoji and text formats
    patterns = [
        (r"\*\*Status:\*\*\s+(?:✅\s+)?(COMPLETE|SUCCESS|DONE|PASS)", "pass"),  # With optional emoji
        (r"Status:\s+(?:✅\s+)?(COMPLETE|SUCCESS|DONE|PASS)", "pass"),
        (r"\*\*Segment Status:\*\*\s*(?:\[OK\]|\[PASS\])?\s*(COMPLETE|SUCCESS|DONE)", "pass"),
        (r"Segment Status:\s*(?:\[OK\]|\[PASS\])?\s*(COMPLETE|SUCCESS|DONE)", "pass"),
        (r"COMPLETE\s*-\s*No further work required", "pass"),
        # Recognize various iterative-builder completion formats
        (r"##\s+Segment\s+\d+\s+Complete:", "pass"),  # "## Segment 2 Complete:" (most common)
        (r"##\s+✅\s+Segment\s+\d+\s+Complete:", "pass"),  # "## ✅ Segment 4 Complete:"
        (r"Segment\s+\d+\s+Complete:", "pass"),  # "Segment 2 Complete:"
        (r"##\s+Segment\s+\d+\s+Complete[:\s]+.*?✓", "pass"),  # With checkmark at end
        (r"Segment\s+\d+\s+Complete[:\s]+.*?✓", "pass"),
        (r"Segment\s+\d+\s+completed successfully", "pass"),  # "Segment 1 completed successfully"
        (r"##.*Segment\s+\d+.*-\s*(COMPLETE|SUCCESS)", "pass"),  # "## ✅ Segment 1: ... - COMPLETE"
        (r"##.*Segment\s+\d+.*Report.*SUCCESS", "pass"),  # "## 🎯 Segment 1 Build Report - SUCCESS"
        # Generic completion indicators (when builder doesn't format properly)
        (r"Ready for commit with message:", "pass"),  # "Ready for commit with message: ..."
        (r"test suite is ready for use", "pass"),  # "The test suite is ready for use"
        (r"All.*tests (?:pass|passing)", "pass"),  # "All tests passing"
        (r"\*\*Status:\*\*\s+(IN_PROGRESS|ONGOING)", "partial"),
        (r"Status:\s+(IN_PROGRESS|ONGOING)", "partial"),
    ]

    for pattern, status in patterns:
        match = re.search(pattern, log_text, re.IGNORECASE)
        if match:
            log.warning(
                "Non-standard status format detected: '%s' (mapped to %s)",
                match.group(1), status
            )
            return status

    return "unknown"


def _extract_summary(log_text: str, max_len: int = 300) -> str:
    """Extract the full builder report or a brief summary.

    Looks for the builder report starting with ## Builder Report or similar,
    and captures everything from that point forward (full report for dashboard).
    Falls back to brief summary if no builder report found.
    """
    # Look for builder report markers (capture full report)
    for marker in ("## Builder Report", "## Segment Report", "**Status:**"):
        idx = log_text.find(marker)
        if idx >= 0:
            # Return everything from the builder report onwards (full report)
            return log_text[idx:].strip()

    # Fallback: brief summary for segments without builder report format
    for header in ("### What was built", "## What was built"):
        idx = log_text.find(header)
        if idx >= 0:
            chunk = log_text[idx + len(header):idx + len(header) + max_len]
            return chunk.strip().split("\n\n")[0].strip()

    # Final fallback: last 200 chars
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


def _extract_token_usage(raw_path: Path) -> tuple[int, int, int, int, float]:
    """Parse token counts and cost from the stream-json result event.

    Returns (input_tokens, output_tokens, cache_read_tokens, cache_creation_tokens, cost_usd),
    or (0, 0, 0, 0, 0.0) on any failure.
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
                    return (
                        u.get("input_tokens", 0),
                        u.get("output_tokens", 0),
                        u.get("cache_read_input_tokens", 0),
                        u.get("cache_creation_input_tokens", 0),
                        obj.get("total_cost_usd", 0.0),
                    )
    except Exception:
        pass
    return 0, 0, 0, 0, 0.0


def _extract_cycles_used(raw_path: Path) -> int:
    """Parse cycle count from agent output.

    Looks for patterns like "Cycle 5/10" or "Cycle 3 of 8" in the log.
    Returns the highest cycle number seen, or 0 if none found.
    """
    max_cycle = 0
    try:
        with open(raw_path, encoding="utf-8", errors="replace") as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                # Extract text from stream JSON
                text = _extract_text_from_stream_line(line)
                if not text:
                    continue
                # Match patterns like "Cycle 5/10" or "Cycle 3 of 8"
                match = re.search(r"Cycle\s+(\d+)(?:/|\s+of\s+)\d+", text, re.IGNORECASE)
                if match:
                    cycle_num = int(match.group(1))
                    max_cycle = max(max_cycle, cycle_num)
    except Exception:
        pass
    return max_cycle


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
    partial_context: str | None = None,
) -> tuple[str, str]:
    """Execute a single segment via claude CLI.

    Args:
        cwd: Optional working directory for the subprocess. When isolation_strategy="worktree",
             the orchestrator will pass the worktree path here.
        partial_context: Summary from previous partial attempt to help agent continue.

    Returns (status, summary).
    """
    # Check for pending operator interject
    pending_interject = await state.get_pending_interject(seg.num)
    interject_msg = pending_interject["message"] if pending_interject else None
    interject_id = pending_interject["id"] if pending_interject else None

    prompt = _build_prompt(seg, config, interject=interject_msg, partial_context=partial_context)
    env = _build_env(seg.num, config)

    # Archive previous attempt's logs before starting new attempt
    if attempt_num > 1:
        prev_attempt = attempt_num - 1
        old_raw = log_dir / f"S{seg.num:02d}.stream.jsonl"
        old_human = log_dir / f"S{seg.num:02d}.log"

        # Rename (atomic operation) to archive
        if old_raw.exists():
            archive_raw = log_dir / f"S{seg.num:02d}-attempt{prev_attempt}.stream.jsonl"
            old_raw.rename(archive_raw)

        if old_human.exists():
            archive_human = log_dir / f"S{seg.num:02d}-attempt{prev_attempt}.log"
            old_human.rename(archive_human)

    # Continue with standard naming (unchanged)
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

        # Mark interject as consumed after successful process spawn
        if interject_id:
            await state.consume_interject(interject_id)
            await state.log_event(
                "interject_consumed",
                f"S{seg.num:02d} restarted with operator message",
                severity="info"
            )

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
            tokens_in, tokens_out, cache_read, cache_creation, cost = _extract_token_usage(raw_log)
            cycles_used = _extract_cycles_used(raw_log)
            await state.record_attempt(
                seg.num, attempt_num, started_at, finished_at,
                "timeout", f"Killed after {segment_timeout}s",
                tokens_in, tokens_out, cycles_used, cache_read, cache_creation, cost,
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
            "failed", str(exc), 0, 0, 0, 0, 0, 0.0,
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

    tokens_in, tokens_out, cache_read, cache_creation, cost = _extract_token_usage(raw_log)
    cycles_used = _extract_cycles_used(raw_log)
    await state.record_attempt(
        seg.num, attempt_num, started_at, finished_at,
        status, summary, tokens_in, tokens_out, cycles_used, cache_read, cache_creation, cost,
    )

    log.info("S%02d finished: %s", seg.num, status.upper())
    return status, summary
