"""Orchestrator CLI: run, status, dry-run."""

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
from .planner import Segment, load_plan
from .runner import run_segment
from .state import StateDB

log = logging.getLogger("orchestrate")


async def _run_gate(config: OrchestrateConfig, log_dir: Path, wave: int) -> tuple[bool, str]:
    """Run the configured gate command after a wave, streaming output to a log file."""
    if not config.gate_command:
        return True, "no gate configured"
    gate_log = log_dir / f"gate-W{wave}.log"
    log.info("Running gate: %s", config.gate_command)

    proc = await asyncio.create_subprocess_shell(
        config.gate_command,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.STDOUT,
    )

    lines: list[str] = []
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
    passed = proc.returncode == 0
    return passed, "\n".join(lines)


async def _heartbeat_loop(
    state: StateDB,
    notifier: Notifier,
    interval: int,
    stop_event: asyncio.Event,
) -> None:
    """Periodically send a status summary notification."""
    if interval <= 0:
        return
    while not stop_event.is_set():
        try:
            await asyncio.wait_for(stop_event.wait(), timeout=interval)
            break  # stop_event was set
        except asyncio.TimeoutError:
            pass  # interval elapsed, send heartbeat

        progress = await state.progress()
        total = sum(progress.values())
        running = progress.get("running", 0)
        passed = progress.get("pass", 0)
        current_wave = await state.get_meta("current_wave") or "?"
        summary = (
            f"Wave {current_wave} | "
            f"{passed}/{total} passed, {running} running | "
            f"Breakdown: {json.dumps(progress)}"
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


async def _run_wave(
    wave: int,
    segments: list[Segment],
    config: OrchestrateConfig,
    state: StateDB,
    log_dir: Path,
    shutting_down: asyncio.Event,
) -> list[tuple[int, str]]:
    """Execute all segments in a wave with bounded parallelism.

    Returns list of (segment_num, status).
    """
    sem = asyncio.Semaphore(config.max_parallel)
    results: list[tuple[int, str]] = []

    async def _run_one(seg: Segment) -> tuple[int, str]:
        if shutting_down.is_set():
            return seg.num, "skipped"
        async with sem:
            if shutting_down.is_set():
                return seg.num, "skipped"

            attempts = 0
            status = "failed"
            while attempts <= config.max_retries:
                attempts = await state.increment_attempts(seg.num)
                status, _ = await run_segment(seg, config, state, log_dir)
                if status in ("pass", "timeout"):
                    break
                if attempts > config.max_retries:
                    break
                log.info("S%02d retrying (attempt %d/%d)", seg.num, attempts, config.max_retries)
                await state.log_event("segment_retry", f"S{seg.num:02d} attempt {attempts}")

            return seg.num, status

    tasks = [asyncio.create_task(_run_one(seg)) for seg in segments]
    done = await asyncio.gather(*tasks, return_exceptions=True)
    for item in done:
        if isinstance(item, Exception):
            log.error("Wave %d segment error: %s", wave, item)
            results.append((0, "error"))
        else:
            results.append(item)
    return results


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
    monitor = MonitorServer(state, log_dir, config.monitor_port)

    shutting_down = asyncio.Event()
    worker_stop = asyncio.Event()

    # Signal handlers
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, lambda: _handle_shutdown(shutting_down, worker_stop))

    await monitor.start()
    await notifier.started(meta.title, len(segments), max_wave)

    # Banner
    print(f"\n{'='*60}")
    print(f"  ORCHESTRATOR — {meta.title}")
    print(f"  {len(segments)} segments in {max_wave} waves")
    print(f"  Parallelism: {config.max_parallel} | Timeout: {config.segment_timeout}s")
    if config.monitor_port:
        print(f"  Dashboard: http://localhost:{config.monitor_port}")
    print(f"{'='*60}\n")

    heartbeat_task = asyncio.create_task(
        _heartbeat_loop(state, notifier, config.heartbeat_interval, worker_stop)
    )
    notif_task = asyncio.create_task(
        _notification_worker(notifier, state, worker_stop)
    )

    try:
        for wave_num in range(1, max_wave + 1):
            if shutting_down.is_set():
                log.warning("Shutting down, skipping wave %d+", wave_num)
                break

            wave_segs = waves.get(wave_num, [])
            if not wave_segs:
                continue

            # Skip segments that already passed (resume support)
            pending = []
            for s in wave_segs:
                seg = await state.get_segment(s.num)
                if seg and seg.status not in ("pass",):
                    pending.append(s)

            if not pending:
                log.info("Wave %d: all segments already passed, skipping", wave_num)
                continue

            await state.set_meta("current_wave", str(wave_num))
            seg_nums = [s.num for s in pending]
            await state.log_event("wave_start", f"Wave {wave_num}/{max_wave}: {seg_nums}")

            print(f"\n{'━'*50}")
            print(f"  Wave {wave_num}/{max_wave} — {len(pending)} segments: "
                  f"{', '.join(f'S{s.num:02d}' for s in pending)}")
            print(f"{'━'*50}")

            results = await _run_wave(
                wave_num, pending, config, state, log_dir, shutting_down
            )

            # Batched wave completion notification
            await notifier.wave_complete(wave_num, max_wave, results)
            # Individual notifications for non-passing segments
            for seg_num, status in results:
                if status not in ("pass", "skipped"):
                    seg = next((s for s in pending if s.num == seg_num), None)
                    if seg:
                        await notifier.segment_complete(seg_num, seg.title, status, "")

            # Wave summary
            passed = sum(1 for _, s in results if s == "pass")
            failed = sum(1 for _, s in results if s not in ("pass", "skipped"))
            print(f"  Wave {wave_num} complete: {passed} passed, {failed} failed")

            # Gate check
            if config.gate_command and not shutting_down.is_set():
                gate_ok, gate_output = await _run_gate(config, log_dir, wave_num)
                await state.log_event(
                    "gate_result",
                    f"Wave {wave_num} gate: {'PASS' if gate_ok else 'FAIL'}",
                )
                await notifier.gate_result(wave_num, gate_ok, gate_output)
                if not gate_ok:
                    log.error("Gate failed after wave %d, stopping", wave_num)
                    await notifier.error(f"Gate failed after wave {wave_num}. Stopping.")
                    break

    except Exception as exc:
        log.exception("Orchestration error")
        await notifier.error(str(exc))
    finally:
        worker_stop.set()
        await heartbeat_task
        await notif_task

        progress = await state.progress()
        await state.log_event("run_complete", json.dumps(progress))
        await notifier.finished(meta.title, progress)

        await monitor.stop()
        await state.close()

        print(f"\n{'='*60}")
        print(f"  ORCHESTRATION COMPLETE")
        print(f"  Results: {progress}")
        print(f"{'='*60}\n")


def _handle_shutdown(shutting_down: asyncio.Event, worker_stop: asyncio.Event) -> None:
    log.warning("Received shutdown signal")
    shutting_down.set()
    worker_stop.set()


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
        icon = {"PASS": "✅", "RUNNING": "🔄", "PENDING": "⏳", "FAILED": "❌",
                "BLOCKED": "🚫", "PARTIAL": "⚠️", "TIMEOUT": "⏰"}.get(status, "❓")
        print(f"  {icon} S{seg['num']:02d} [{status:8s}] {seg['title']}")
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


if __name__ == "__main__":
    main()
