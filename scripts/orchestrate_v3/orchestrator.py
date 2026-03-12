"""Orchestrator coordinator class for managing wave execution."""

from __future__ import annotations

import json
import logging
import time
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .config import OrchestrateConfig
    from .monitor import MonitorServer
    from .notify import Notifier
    from .planner import PlanMeta, Segment
    from .recovery import RecoveryAgent
    from .signal_handler import SignalHandler
    from .state import StateDB, SegmentRow
    from .worktree_pool import WorktreePool

from .wave_runner import WaveRunner

log = logging.getLogger("orchestrate")


class Orchestrator:
    """High-level orchestration coordinator with recovery and gate checks."""

    def __init__(
        self,
        state: StateDB,
        config: OrchestrateConfig,
        notifier: Notifier,
        monitor: MonitorServer,
        signal_handler: SignalHandler,
        log_dir: Path,
        recovery_agent: RecoveryAgent | None = None,
        pool: WorktreePool | None = None,
        running_pids: dict[int, int] | None = None,
    ):
        self.state = state
        self.config = config
        self.notifier = notifier
        self.monitor = monitor
        self.signal_handler = signal_handler
        self.recovery_agent = recovery_agent
        self.pool = pool
        self.log_dir = log_dir
        self.wave_runner = WaveRunner(
            state, config, notifier, log_dir, pool, running_pids
        )

    async def run(
        self,
        segments: list[Segment],
        waves: dict[int, list[Segment]],
        max_wave: int,
        meta: PlanMeta,
        _run_gate,
        _pre_wave_health_check,
        _wait_for_network,
    ) -> None:
        """Main orchestration loop with recovery and gate checks.

        Args:
            segments: All segments in the plan
            waves: Mapping of wave number to segments
            max_wave: Maximum wave number
            meta: Plan metadata
            _run_gate: Function to run gate checks
            _pre_wave_health_check: Function for pre-flight health checks
            _wait_for_network: Function to wait for network connectivity
        """
        await self.monitor.start()
        await self.notifier.started(meta.title, len(segments), max_wave)

        try:
            for wave_num in range(1, max_wave + 1):
                if self.signal_handler.is_shutting_down():
                    log.warning("Shutting down, skipping wave %d+", wave_num)
                    break

                wave_segs = waves.get(wave_num, [])
                if not wave_segs:
                    continue

                # Resume support: skip already-passed segments
                pending = await self._filter_pending_segments(wave_segs)
                if not pending:
                    log.info("Wave %d: all segments already passed, skipping", wave_num)
                    continue

                await self.state.set_meta("current_wave", str(wave_num))
                seg_nums = [s.num for s in pending]
                await self.state.log_event("wave_start", f"Wave {wave_num}/{max_wave}: {seg_nums}")

                print(f"\n{'━'*50}")
                print(f"  Wave {wave_num}/{max_wave} — {len(pending)} segments: "
                      f"{', '.join(f'S{s.num:02d}' for s in pending)}")
                print(f"{'━'*50}")

                await _wait_for_network(self.notifier, self.config.network_retry_max)

                # Pre-flight health check
                if self.config.enable_preflight_checks:
                    healthy, errors = await _pre_wave_health_check(wave_num, self.config, self.state)

                    if not healthy:
                        await self._handle_preflight_failure(wave_num, errors)
                        break

                # Execute wave
                results = await self.wave_runner.execute(
                    wave_num,
                    pending,
                    self.signal_handler.shutting_down,
                    segments,
                )

                # Batched wave completion notification
                await self.notifier.wave_complete(wave_num, max_wave, results)
                # Individual notifications for non-passing segments
                for seg_num, status in results:
                    if status not in ("pass", "merged", "skipped"):
                        seg = next((s for s in pending if s.num == seg_num), None)
                        if seg:
                            await self.notifier.segment_complete(seg_num, seg.title, status, "")

                # Wave summary
                passed = sum(1 for _, s in results if s in ("pass", "merged"))
                failed = sum(1 for _, s in results if s not in ("pass", "skipped"))
                print(f"  Wave {wave_num} complete: {passed} passed, {failed} failed")

                # Recovery: Auto-retry cascade victims
                if self.config.recovery_enabled and self.recovery_agent:
                    await self._attempt_recovery(
                        wave_num, pending, results, segments
                    )

                # Gate check
                if self.config.gate_command:
                    gate_ok = await self._run_gate_check(wave_num, _run_gate)
                    if not gate_ok and self.config.gate_required:
                        log.error("Gate failed after wave %d, stopping", wave_num)
                        await self.notifier.error(f"Gate failed after wave {wave_num}. Stopping.")
                        break

        except Exception as exc:
            log.exception("Orchestration error")
            await self.notifier.error(str(exc))
            raise
        finally:
            await self.cleanup(meta)

    async def _filter_pending_segments(self, wave_segs: list[Segment]) -> list[Segment]:
        """Filter segments to only those that need to run."""
        pending = []
        for s in wave_segs:
            seg = await self.state.get_segment(s.num)
            if seg and seg["status"] not in ("pass", "merged", "skipped"):
                pending.append(s)
        return pending

    async def _handle_preflight_failure(self, wave_num: int, errors: list[str]) -> None:
        """Handle pre-flight health check failure."""
        log.error(
            f"Wave {wave_num} blocked by workspace errors. "
            f"Fix errors and resume with: orchestrate resume"
        )

        # Notify operator
        error_summary = (
            f"Wave {wave_num} pre-flight failed: {len(errors)} compilation errors. "
            f"First error: {errors[0][:100] if errors else 'unknown'}"
        )
        await self.notifier.error(error_summary)

    async def _attempt_recovery(
        self,
        wave_num: int,
        pending: list[Segment],
        results: list[tuple[int, str]],
        segments: list[Segment],
    ) -> None:
        """Attempt recovery for cascade victims."""
        if self.signal_handler.is_shutting_down():
            return

        # Check if wave has any failures to trigger recovery
        has_failures = any(status in ("partial", "blocked") for _, status in results)
        if not has_failures:
            return

        log.info("Recovery: Wave has failures, checking workspace health...")

        # Check workspace health
        healthy, errors = await self.recovery_agent.check_workspace_health()
        if not healthy:
            log.warning(
                "Recovery: Workspace health check failed, skipping victim retry. Errors: %s",
                errors[:3] if len(errors) > 3 else errors,
            )
            await self.state.log_event(
                "recovery_skipped",
                f"Wave {wave_num}: workspace unhealthy, {len(errors)} errors",
            )
            return

        log.info("Recovery: Workspace is healthy, identifying cascade victims...")

        # Get segment rows for victim identification
        wave_seg_rows = []
        for seg in pending:
            seg_data = await self.state.get_segment(seg.num)
            if seg_data:
                from .state import SegmentRow
                wave_seg_rows.append(SegmentRow(
                    num=seg.num,
                    slug=seg.slug,
                    title=seg.title,
                    wave=wave_num,
                    status=seg_data["status"],
                    attempts=seg_data["attempts"],
                ))

        # Identify victims
        victims = await self.recovery_agent.identify_cascade_victims(wave_seg_rows, self.log_dir)

        if not victims:
            log.info("Recovery: No cascade victims identified")
            return

        # Apply circuit breaker: filter by max_attempts
        filtered_victims = []
        for seg_num in victims:
            seg_data = await self.state.get_segment(seg_num)
            if seg_data and seg_data["attempts"] < self.config.recovery_max_attempts:
                filtered_victims.append(seg_num)
            else:
                log.info(
                    "Recovery: S%02d filtered by circuit breaker (attempts: %d >= max: %d)",
                    seg_num,
                    seg_data["attempts"] if seg_data else 0,
                    self.config.recovery_max_attempts,
                )

        if not filtered_victims:
            log.info("Recovery: No victims to retry after circuit breaker filter")
            return

        log.info(
            "Recovery: Retrying %d victims: %s",
            len(filtered_victims),
            filtered_victims,
        )

        # Use wave_runner to execute recovery mini-wave
        recovery_results = await self._run_recovery_wave(
            filtered_victims,
            segments,
            wave_num,
        )

        # Update results with recovery outcomes
        passed = sum(1 for _, s in recovery_results if s == "pass")
        failed = sum(1 for _, s in recovery_results if s not in ("pass", "skipped"))
        print(f"  After recovery: {passed} passed, {failed} failed")

    async def _run_recovery_wave(
        self,
        victim_segs: list[int],
        all_segments: list[Segment],
        wave_num: int,
    ) -> list[tuple[int, str]]:
        """Run a mini-wave to retry victim segments after recovery check.

        Args:
            victim_segs: List of segment numbers to retry
            all_segments: All segments in the plan (to look up Segment objects)
            wave_num: Current wave number (for logging)

        Returns:
            List of (segment_num, status) tuples
        """
        log.info("Recovery: Running recovery mini-wave for %d victims: %s", len(victim_segs), victim_segs)
        await self.state.log_event("recovery_triggered", f"Wave {wave_num}: retrying {len(victim_segs)} victims: {victim_segs}")

        # Look up Segment objects for victim segment numbers
        segments_to_retry = []
        for seg_num in victim_segs:
            seg = next((s for s in all_segments if s.num == seg_num), None)
            if seg:
                segments_to_retry.append(seg)
            else:
                log.warning("Recovery: Segment S%02d not found in plan, skipping", seg_num)

        if not segments_to_retry:
            log.warning("Recovery: No valid segments found to retry")
            return []

        # Reset segment status to pending for recovery retry
        for seg in segments_to_retry:
            await self.state.set_status(seg.num, "pending")
            await self.state.log_event("recovery_retry", f"S{seg.num:02d} reset to pending for recovery")

        # Run the recovery wave (using wave_runner)
        results = await self.wave_runner.execute(
            wave_num,
            segments_to_retry,
            self.signal_handler.shutting_down,
            all_segments,
        )

        # Log recovery results
        passed = sum(1 for _, s in results if s == "pass")
        failed = sum(1 for _, s in results if s not in ("pass", "skipped"))
        await self.state.log_event(
            "recovery_complete",
            f"Wave {wave_num} recovery: {passed} passed, {failed} failed"
        )
        log.info("Recovery mini-wave complete: %d passed, %d failed", passed, failed)

        return results

    async def _run_gate_check(self, wave_num: int, _run_gate) -> bool:
        """Run gate check after wave."""
        if self.signal_handler.is_shutting_down():
            return True

        gate_ok, gate_output = await _run_gate(self.config, self.log_dir, wave_num)
        await self.state.log_event(
            "gate_result",
            f"Wave {wave_num} gate: {'PASS' if gate_ok else 'FAIL'}",
        )
        await self.notifier.gate_result(wave_num, gate_ok, gate_output)
        return gate_ok

    async def cleanup(self, meta: PlanMeta) -> None:
        """Clean up resources."""
        progress = await self.state.progress()
        await self.state.log_event("run_complete", json.dumps(progress))
        await self.notifier.finished(meta.title, progress)

        if self.pool:
            await self.pool.cleanup()
            log.info("Cleaned up worktree pool")

        await self.monitor.stop()
        await self.state.close()

        print(f"\n{'='*60}")
        print(f"  ORCHESTRATION COMPLETE")
        print(f"  Results: {progress}")
        print(f"{'='*60}\n")
