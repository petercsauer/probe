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
from .notify import Notifier
from .planner import Segment, load_plan
from .runner import run_segment
from .state import StateDB

log = logging.getLogger("orchestrate")


async def _run_gate(config: OrchestrateConfig, log_dir: Path, wave: int) -> tuple[bool, str]:
    """Run the configured gate command after a wave."""
    if not config.gate_command:
        return True, "no gate configured"
    gate_log = log_dir / f"gate-wave{wave}.log"
    log.info("Running gate: %s", config.gate_command)
    proc = await asyncio.create_subprocess_shell(
        config.gate_command,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.STDOUT,
    )
    stdout, _ = await proc.communicate()
    output = stdout.decode(errors="replace")
    gate_log.write_text(output, encoding="utf-8")
    passed = proc.returncode == 0
    return passed, output


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

        progress = state.progress()
        total = sum(progress.values())
        running = progress.get("running", 0)
        passed = progress.get("pass", 0)
        current_wave = state.get_meta("current_wave") or "?"
        summary = (
            f"Wave {current_wave} | "
            f"{passed}/{total} passed, {running} running | "
            f"Breakdown: {json.dumps(progress)}"
        )
        await notifier.heartbeat(summary)


async def _run_wave(
    wave: int,
    segments: list[Segment],
    config: OrchestrateConfig,
    state: StateDB,
    notifier: Notifier,
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
            summary = ""
            while attempts <= config.max_retries:
                attempts = state.increment_attempts(seg.num)
                status, summary = await run_segment(seg, config, state, log_dir)
                if status in ("pass", "timeout"):
                    break
                if attempts > config.max_retries:
                    break
                log.info("S%02d retrying (attempt %d/%d)", seg.num, attempts, config.max_retries)
                state.log_event("segment_retry", f"S{seg.num:02d} attempt {attempts}")

            await notifier.segment_complete(seg.num, seg.title, status, summary)
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

    # State
    db_path = plan_dir / "state.db"
    state = StateDB(db_path)
    state.init_segments(segments)
    state.set_meta("plan_title", meta.title)
    state.set_meta("plan_goal", meta.goal)
    state.set_meta("total_segments", str(len(segments)))
    state.set_meta("max_wave", str(max_wave))

    # Migrate from old bash script's execution-state.json if present
    old_state = plan_dir / "execution-state.json"
    migrated = state.migrate_from_json(old_state)
    if migrated:
        log.info("Migrated %d segment statuses from %s", migrated, old_state.name)

    # Reset stale "running" segments from a previous crashed run
    stale = state.reset_stale_running()
    if stale:
        log.info("Reset %d stale running segments to pending", stale)

    state.set_meta("started_at", str(time.time()))
    state.log_event("run_start", f"{len(segments)} segments, {max_wave} waves")

    notifier = Notifier(config)
    monitor = MonitorServer(state, log_dir, config.monitor_port)

    shutting_down = asyncio.Event()
    heartbeat_stop = asyncio.Event()

    # Signal handlers
    loop = asyncio.get_running_loop()
    for sig in (signal.SIGINT, signal.SIGTERM):
        loop.add_signal_handler(sig, lambda: _handle_shutdown(shutting_down, heartbeat_stop))

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
        _heartbeat_loop(state, notifier, config.heartbeat_interval, heartbeat_stop)
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
            pending = [s for s in wave_segs if state.get_segment(s.num).status not in ("pass",)]
            if not pending:
                log.info("Wave %d: all segments already passed, skipping", wave_num)
                continue

            state.set_meta("current_wave", str(wave_num))
            seg_nums = [s.num for s in pending]
            state.log_event("wave_start", f"Wave {wave_num}/{max_wave}: {seg_nums}")
            await notifier.wave_start(wave_num, max_wave, seg_nums)

            print(f"\n{'━'*50}")
            print(f"  Wave {wave_num}/{max_wave} — {len(pending)} segments: "
                  f"{', '.join(f'S{s.num:02d}' for s in pending)}")
            print(f"{'━'*50}")

            results = await _run_wave(
                wave_num, pending, config, state, notifier, log_dir, shutting_down
            )

            # Wave summary
            passed = sum(1 for _, s in results if s == "pass")
            failed = sum(1 for _, s in results if s not in ("pass", "skipped"))
            print(f"  Wave {wave_num} complete: {passed} passed, {failed} failed")

            # Gate check
            if config.gate_command and not shutting_down.is_set():
                gate_ok, gate_output = await _run_gate(config, log_dir, wave_num)
                state.log_event(
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
        heartbeat_stop.set()
        await heartbeat_task

        progress = state.progress()
        state.log_event("run_complete", json.dumps(progress))
        await notifier.finished(meta.title, progress)

        await monitor.stop()
        state.close()

        print(f"\n{'='*60}")
        print(f"  ORCHESTRATION COMPLETE")
        print(f"  Results: {progress}")
        print(f"{'='*60}\n")


def _handle_shutdown(shutting_down: asyncio.Event, heartbeat_stop: asyncio.Event) -> None:
    log.warning("Received shutdown signal")
    shutting_down.set()
    heartbeat_stop.set()


def cmd_status(plan_dir: Path) -> None:
    """Print current state summary."""
    db_path = plan_dir / "state.db"
    if not db_path.exists():
        print("No state.db found. Has the orchestrator been run?")
        return
    state = StateDB(db_path)
    data = state.all_as_dict()
    state.close()

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
