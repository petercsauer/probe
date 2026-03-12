"""Orchestrator v3 CLI: run, status, dry-run."""

from __future__ import annotations

import argparse
import asyncio
import json
import logging
import os
import signal
import sys
import time
from pathlib import Path

from .config import OrchestrateConfig
from .monitor import MonitorServer
from .notify import _send_ntfy, Notifier
from .orchestrator import Orchestrator
from .planner import Segment, load_plan
from .recovery import RecoveryAgent
from .runner import CircuitBreaker, run_segment
from .signal_handler import SignalHandler
from .state import StateDB
from .worktree_pool import WorktreePool

log = logging.getLogger("orchestrate")

# Module-level running PIDs registry (seg_num → OS PID).
# Single dict shared between orchestration and the monitor server so the
# /api/control kill action can find live processes.  Thread-safe because
# asyncio is single-threaded.
_running_pids: dict[int, int] = {}


async def _run_gate(config: OrchestrateConfig, log_dir: Path, wave: int) -> tuple[bool, str]:
    """Run the configured gate command after a wave, streaming output to a log file.

    Gate execution has a timeout (default 1800s / 30min) to prevent deadlocks.
    """
    if not config.gate_command:
        return True, "no gate configured"
    gate_log = log_dir / f"gate-W{wave}.log"
    gate_timeout = config.gate_timeout
    log.info("Running gate: %s (timeout: %ds)", config.gate_command, gate_timeout)

    try:
        proc = await asyncio.create_subprocess_shell(
            config.gate_command,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.STDOUT,
        )

        lines: list[str] = []

        async def _stream_output():
            """Stream output to file with line-by-line reading."""
            with open(gate_log, "w", encoding="utf-8") as f:
                while True:
                    raw = await proc.stdout.readline()
                    if not raw:
                        break
                    line = raw.decode(errors="replace").rstrip()
                    lines.append(line)
                    f.write(line + "\n")
                    f.flush()
            await proc.wait()

        # Run with timeout
        await asyncio.wait_for(_stream_output(), timeout=gate_timeout)

        passed = proc.returncode == 0
        return passed, "\n".join(lines)

    except asyncio.TimeoutError:
        # Gate timed out - kill the process
        log.error("Gate timed out after %ds, killing process", gate_timeout)
        try:
            proc.kill()
            await proc.wait()
        except Exception:
            pass

        # Write timeout message to log
        timeout_msg = f"GATE TIMEOUT: Execution exceeded {gate_timeout}s limit"
        lines.append(timeout_msg)
        with open(gate_log, "a", encoding="utf-8") as f:
            f.write(f"\n{timeout_msg}\n")

        return False, "\n".join(lines)

    except Exception as e:
        # Gate execution failed
        log.error("Gate execution failed: %s", e)
        error_msg = f"GATE ERROR: {str(e)}"
        with open(gate_log, "a", encoding="utf-8") as f:
            f.write(f"\n{error_msg}\n")
        return False, error_msg


async def _claude_summarise(context: str, config: "OrchestrateConfig") -> str:
    """Ask Claude for a concise push-notification summary of current run state.

    Falls back to an empty string on any failure so the caller can use a
    plain-text fallback instead.
    """
    prompt = (
        "Summarise this automated code-build orchestration run in 2-3 sentences "
        "suitable for a mobile push notification. Be concrete: name the segments "
        "and what they are doing based on last_activity. "
        "Do NOT include preamble, headers, or bullet points — plain prose only.\n\n"
        f"{context}"
    )
    env = dict(os.environ)
    env.update(config.auth_env)
    try:
        proc = await asyncio.create_subprocess_exec(
            "claude", "-p", prompt,
            "--dangerously-skip-permissions",
            "--max-turns", "1",
            "--output-format", "text",
            stdin=asyncio.subprocess.DEVNULL,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.DEVNULL,
            env=env,
            start_new_session=True,
            limit=2**20,
        )
        try:
            stdout, _ = await asyncio.wait_for(proc.communicate(), timeout=45)
            return stdout.decode("utf-8", errors="replace").strip()
        except asyncio.TimeoutError:
            try:
                proc.kill()
            except Exception:
                pass
    except Exception:
        pass
    return ""


async def _heartbeat_loop(
    state: StateDB,
    notifier: Notifier,
    interval: int,
    stop_event: asyncio.Event,
    config: "OrchestrateConfig | None" = None,
) -> None:
    """Periodically send an AI-generated status summary notification."""
    if interval <= 0:
        return
    while not stop_event.is_set():
        try:
            await asyncio.wait_for(stop_event.wait(), timeout=interval)
            break  # stop_event was set
        except asyncio.TimeoutError:
            pass  # interval elapsed — send heartbeat

        progress = await state.progress()
        total = sum(progress.values())
        passed = progress.get("pass", 0)
        current_wave = await state.get_meta("current_wave") or "?"

        # Build rich context from live segment state
        all_data = await state.all_as_dict()
        segs = all_data.get("segments", [])
        running_segs = [s for s in segs if s.get("status") == "running"]
        failed_segs  = [s for s in segs if s.get("status") in ("failed", "blocked", "partial", "timeout")]

        context_lines = [
            f"Wave {current_wave} | Progress: {json.dumps(progress)} ({passed}/{total} segments passed)",
        ]
        if running_segs:
            context_lines.append("Currently running:")
            for s in running_segs:
                elapsed = ""
                if s.get("started_at"):
                    elapsed = f" ({int(time.time() - s['started_at'])}s elapsed)"
                activity = (s.get("last_activity") or "no activity recorded yet")[:300]
                context_lines.append(f"  S{s['num']:02d} {s['title']}{elapsed} — last activity: {activity}")
        if failed_segs:
            context_lines.append("Failed/blocked:")
            for s in failed_segs:
                context_lines.append(f"  S{s['num']:02d} {s['title']} [{s['status']}]")

        context = "\n".join(context_lines)

        # Try to get an AI summary; fall back to plain text
        summary = ""
        if config and running_segs:
            summary = await _claude_summarise(context, config)
        if not summary:
            summary = (
                f"Wave {current_wave} | {passed}/{total} passed, "
                f"{len(running_segs)} running"
                + (f", {len(failed_segs)} failed" if failed_segs else "")
            )

        await notifier.heartbeat(summary)


async def _notification_worker(
    notifier: Notifier,
    state: StateDB,
    stop_event: asyncio.Event,
    poll_interval: int = 10,
) -> None:
    """Poll the notification outbox and deliver pending messages with backoff."""
    retry_delays = notifier._config.notify_retry_delays
    while not stop_event.is_set():
        try:
            pending = await state.get_pending_notifications(notifier._max_attempts)
            for notif in pending:
                if notif["attempts"] > 0 and notif.get("last_attempt_at"):
                    delay = retry_delays[min(notif["attempts"] - 1, len(retry_delays) - 1)]
                    if (time.time() - notif["last_attempt_at"]) < delay:
                        continue
                ok = await _send_ntfy(
                    notifier._topic,
                    notif["message"],
                    priority=notif.get("priority", "default"),
                    click_url=notifier._click_url,
                )
                if ok:
                    await state.mark_notification_sent(notif["id"])
                else:
                    await state.mark_notification_failed(notif["id"], "HTTP error")
        except Exception:
            log.exception("Notification worker error")
        try:
            await asyncio.wait_for(stop_event.wait(), timeout=poll_interval)
        except asyncio.TimeoutError:
            pass


async def _wait_for_network(notifier, max_wait: int = 600) -> None:
    """Poll https://api.anthropic.com until reachable or max_wait seconds elapsed."""
    import httpx  # noqa: PLC0415
    waited, notified, delay = 0, False, 10
    while True:
        try:
            async with httpx.AsyncClient(timeout=5) as c:
                await c.get("https://api.anthropic.com")
            return  # reachable
        except Exception:
            pass
        waited += delay
        if waited >= max_wait:
            log.warning("Network unreachable for %ds, proceeding anyway", max_wait)
            return
        if not notified and waited >= 60 and notifier:
            try:
                await notifier.network_down(waited)
            except Exception:
                pass
            notified = True
        await asyncio.sleep(delay)
        delay = min(delay * 2, 60)


async def _pre_wave_health_check(
    wave: int,
    config: OrchestrateConfig,
    state: StateDB,
) -> tuple[bool, list[str]]:
    """Validate workspace health before launching wave segments.

    Args:
        wave: Wave number for logging
        config: Orchestration config
        state: State database for logging events

    Returns:
        (healthy: bool, errors: list[str])

    If unhealthy, errors list contains compiler error messages.
    """
    if not getattr(config, 'enable_preflight_checks', True):
        log.debug("Pre-flight checks disabled, skipping")
        return True, []

    log.info(f"Running pre-flight health check for wave {wave}")
    await state.log_event("preflight_check", f"Wave {wave} pre-flight check starting")

    try:
        # Create temporary RecoveryAgent to check workspace health
        recovery = RecoveryAgent(state, config)
        healthy, errors = await recovery.check_workspace_health()

        if healthy:
            log.info(f"[PASS] Pre-flight check passed for wave {wave}")
            await state.log_event("preflight_pass", f"Wave {wave} pre-flight check passed")
        else:
            log.error(
                f"[FAIL] Pre-flight check failed for wave {wave}: "
                f"{len(errors)} errors detected"
            )
            # Log first 5 errors for context
            for error in errors[:5]:
                log.error(f"  - {error}")
            if len(errors) > 5:
                log.error(f"  ... and {len(errors) - 5} more")

            await state.log_event(
                "preflight_fail",
                f"Wave {wave} pre-flight check failed with {len(errors)} errors"
            )

        return healthy, errors

    except asyncio.TimeoutError:
        timeout = getattr(config, 'preflight_timeout', 120)
        log.error(f"Pre-flight check timed out after {timeout}s")
        await state.log_event("preflight_timeout", f"Wave {wave} pre-flight check timed out")
        return False, [f"Health check timed out after {timeout}s"]
    except Exception as e:
        log.error(f"Pre-flight check failed with exception: {e}")
        await state.log_event("preflight_error", f"Wave {wave} pre-flight check error: {e}")
        return False, [f"Health check exception: {str(e)}"]




async def orchestrate(plan_dir: Path, monitor_port: int | None = None) -> None:
    """Main orchestration loop."""
    plan_dir = plan_dir.resolve()
    config = OrchestrateConfig.load(plan_dir)

    if monitor_port is not None:
        config.monitor_port = monitor_port

    meta, segments = load_plan(plan_dir)
    max_wave = max(s.wave for s in segments)
    waves: dict[int, list[Segment]] = {}
    for s in segments:
        waves.setdefault(s.wave, []).append(s)

    # Log directory
    log_dir = plan_dir / "logs"
    log_dir.mkdir(exist_ok=True)

    # Exclusive lock: prevent two orchestrators running against the same plan
    lock_path = plan_dir / "orchestrator.lock"
    try:
        lock_fd = os.open(str(lock_path), os.O_CREAT | os.O_EXCL | os.O_WRONLY)
        os.write(lock_fd, str(os.getpid()).encode())
        os.close(lock_fd)
    except FileExistsError:
        # Check if the PID inside is still alive
        try:
            old_pid = int(lock_path.read_text().strip())
            os.kill(old_pid, 0)  # raises if not alive
            print(f"ERROR: Orchestrator already running (PID {old_pid}). "
                  f"Kill it first or remove {lock_path}", file=sys.stderr)
            sys.exit(1)
        except (ProcessLookupError, ValueError):
            # Stale lock — take it over
            lock_path.write_text(str(os.getpid()))

    try:
        await _orchestrate_inner(
            plan_dir, config, meta, segments, waves, max_wave, log_dir
        )
    finally:
        lock_path.unlink(missing_ok=True)


async def _orchestrate_inner(
    plan_dir: Path,
    config: OrchestrateConfig,
    meta,
    segments: list[Segment],
    waves: dict[int, list[Segment]],
    max_wave: int,
    log_dir: Path,
) -> None:
    """Core orchestration logic (called after lock is acquired)."""
    # State
    db_path = plan_dir / "state.db"
    state = await StateDB.create(db_path)
    await state.init_segments(segments)
    await state.set_meta("plan_title", meta.title)
    await state.set_meta("plan_goal", meta.goal)
    await state.set_meta("total_segments", str(len(segments)))
    await state.set_meta("max_wave", str(max_wave))

    # Migrate from old bash script's execution-state.json if present
    old_state = plan_dir / "execution-state.json"
    migrated = await state.migrate_from_json(old_state)
    if migrated:
        log.info("Migrated %d segment statuses from %s", migrated, old_state.name)

    # Reset stale "running" segments from a previous crashed run
    stale = await state.reset_stale_running()
    if stale:
        log.info("Reset %d stale running segments to pending", stale)

    await state.set_meta("started_at", str(time.time()))
    await state.log_event("run_start", f"{len(segments)} segments, {max_wave} waves")

    notifier = Notifier(config, state)
    monitor = MonitorServer(state, log_dir, config.monitor_port, plan_root=plan_dir, running_pids=_running_pids)

    # NEW: Create worktree pool if isolation strategy requires it
    pool: WorktreePool | None = None
    if config.isolation_strategy == "worktree":
        pool_size = min(config.max_parallel, 4)  # Never exceed 4 worktrees
        pool = WorktreePool(
            repo_root=Path.cwd(),
            pool_size=pool_size,
            target_branch="main",  # TODO: detect current branch
        )
        await pool.create()
        log.info("Created worktree pool with %d worktrees", pool_size)

    # Signal handlers
    signal_handler = SignalHandler()
    loop = asyncio.get_running_loop()
    signal_handler.register_handlers(loop)

    # Banner
    print(f"\n{'='*60}")
    print(f"  ORCHESTRATOR — {meta.title}")
    print(f"  {len(segments)} segments in {max_wave} waves")
    print(f"  Parallelism: {config.max_parallel} | Timeout: {config.segment_timeout}s")
    if config.monitor_port:
        print(f"  Dashboard: http://localhost:{config.monitor_port}")
    print(f"{'='*60}\n")

    heartbeat_task = asyncio.create_task(
        _heartbeat_loop(state, notifier, config.heartbeat_interval, signal_handler.worker_stop, config)
    )
    notif_task = asyncio.create_task(
        _notification_worker(notifier, state, signal_handler.worker_stop)
    )

    # Create recovery agent if enabled
    recovery_agent = RecoveryAgent(state, config) if config.recovery_enabled else None

    # Create orchestrator
    orchestrator = Orchestrator(
        state, config, notifier, monitor,
        signal_handler, log_dir, recovery_agent, pool, _running_pids
    )

    try:
        await orchestrator.run(
            segments, waves, max_wave, meta,
            _run_gate, _pre_wave_health_check,
            _wait_for_network
        )
    finally:
        signal_handler.worker_stop.set()
        await heartbeat_task
        await notif_task




async def _cmd_status_async(plan_dir: Path) -> None:
    db_path = plan_dir / "state.db"
    if not db_path.exists():
        print("No state.db found. Has the orchestrator been run?")
        return
    state = await StateDB.create(db_path)
    data = await state.all_as_dict()
    await state.close()

    print(f"\nPlan: {data['plan_title']}")
    print(f"Wave: {data['current_wave']}/{data['max_wave']}")
    print(f"Progress: {data['progress']}\n")
    for seg in data["segments"]:
        status = seg["status"].upper()
        icon = {"PASS": "[OK]", "RUNNING": "[>]", "PENDING": "[ ]", "FAILED": "[X]",
                "BLOCKED": "[!]", "PARTIAL": "[~]", "TIMEOUT": "[T]"}.get(status, "[?]")
        print(f"  {icon} S{seg['num']:02d} [{status:8s}] {seg['title']}")
        if seg.get("last_seen_at") and seg["status"] == "running":
            age = int(time.time() - seg["last_seen_at"])
            act = (seg.get("last_activity") or "")[:60]
            print(f"    └─ last seen {age}s ago: {act}")
        for att in seg.get("attempts_history", []):
            dur = (
                f"{int(att['finished_at'] - att['started_at'])}s"
                if att.get("finished_at") and att.get("started_at")
                else "--"
            )
            tok = (
                f"{att['tokens_in'] + att['tokens_out']:,} tok"
                if att.get("tokens_in")
                else ""
            )
            print(f"    attempt {att['attempt']}: {att['status']} ({dur}) {tok}")
    print()

    if data["events"]:
        print("Recent events:")
        for ev in data["events"][:10]:
            ts = time.strftime("%H:%M:%S", time.localtime(ev["ts"]))
            print(f"  {ts} {ev['kind']}: {ev['detail'][:80]}")
    print()


def cmd_status(plan_dir: Path) -> None:
    """Print current state summary."""
    asyncio.run(_cmd_status_async(plan_dir))


def cmd_dry_run(plan_dir: Path) -> None:
    """Show computed waves without running anything."""
    config = OrchestrateConfig.load(plan_dir)
    meta, segments = load_plan(plan_dir)
    max_wave = max(s.wave for s in segments)

    print(f"\nPlan: {meta.title}")
    print(f"Goal: {meta.goal}")
    print(f"Segments: {len(segments)} in {max_wave} waves")
    print(f"Parallelism: {config.max_parallel} | Timeout: {config.segment_timeout}s")
    if config.gate_command:
        print(f"Gate: {config.gate_command}")
    if config.isolation_strategy != "none":
        print(f"Isolation: {config.isolation_strategy} {config.isolation_env}")
    print()

    waves: dict[int, list[Segment]] = {}
    for s in segments:
        waves.setdefault(s.wave, []).append(s)

    for w in range(1, max_wave + 1):
        segs = waves.get(w, [])
        print(f"  Wave {w}: {', '.join(f'S{s.num:02d}' for s in segs)}")
        for s in segs:
            deps = f" (depends: {s.depends_on})" if s.depends_on else ""
            print(f"    S{s.num:02d} {s.title}{deps}")
    print()


def main() -> None:
    parser = argparse.ArgumentParser(
        prog="orchestrate",
        description="Plan-independent orchestration tool",
    )
    sub = parser.add_subparsers(dest="command", required=True)

    run_p = sub.add_parser("run", help="Execute a plan")
    run_p.add_argument("plan_dir", type=Path, help="Path to plan directory")
    run_p.add_argument("--monitor", type=int, default=None, metavar="PORT",
                       help="Enable dashboard on PORT")

    status_p = sub.add_parser("status", help="Show current state")
    status_p.add_argument("plan_dir", type=Path, help="Path to plan directory")

    dry_p = sub.add_parser("dry-run", help="Show computed waves")
    dry_p.add_argument("plan_dir", type=Path, help="Path to plan directory")

    skip_p = sub.add_parser("skip", help="Mark a segment as skipped")
    skip_p.add_argument("seg_num", type=int, metavar="SEG_NUM")
    skip_p.add_argument("plan_dir", type=Path)

    retry_p = sub.add_parser("retry", help="Reset a segment for retry")
    retry_p.add_argument("seg_num", type=int, metavar="SEG_NUM")
    retry_p.add_argument("plan_dir", type=Path)

    args = parser.parse_args()

    logging.basicConfig(
        level=logging.INFO,
        format="[%(asctime)s] %(levelname)s %(name)s: %(message)s",
        datefmt="%Y-%m-%d %H:%M:%S",
    )

    if args.command == "run":
        asyncio.run(orchestrate(args.plan_dir, monitor_port=args.monitor))
    elif args.command == "status":
        cmd_status(args.plan_dir)
    elif args.command == "dry-run":
        cmd_dry_run(args.plan_dir)
    elif args.command == "skip":
        async def _do_skip() -> None:
            db = await StateDB.create(args.plan_dir / "state.db")
            await db.set_status(args.seg_num, "skipped")
            await db.log_event("operator_skip", f"S{args.seg_num:02d} skipped via CLI", severity="warn")
            await db.close()
            print(f"S{args.seg_num:02d} marked as skipped")
        asyncio.run(_do_skip())
    elif args.command == "retry":
        async def _do_retry() -> None:
            db = await StateDB.create(args.plan_dir / "state.db")
            await db.reset_for_retry(args.seg_num)
            await db.log_event(
                "operator_retry",
                f"S{args.seg_num:02d} reset for retry via CLI (restart orchestrator to run)",
                severity="warn",
            )
            await db.close()
            print(f"S{args.seg_num:02d} reset to pending — restart orchestrator to run it")
        asyncio.run(_do_retry())


if __name__ == "__main__":
    main()
