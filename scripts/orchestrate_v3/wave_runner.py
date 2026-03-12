"""WaveRunner: Manages parallel execution of segments within a wave."""

from __future__ import annotations

import asyncio
import json
import logging
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .config import OrchestrateConfig
    from .notify import Notifier
    from .planner import Segment
    from .state import StateDB
    from .worktree_pool import WorktreePool

from .segment_executor import SegmentExecutor

log = logging.getLogger("orchestrate")


async def _validate_upstream_dependencies(
    seg: Segment,
    state: StateDB,
) -> tuple[bool, list[str]]:
    """Check that all upstream dependencies passed.

    Args:
        seg: Segment to check
        state: State database

    Returns:
        (can_run: bool, blocking_segments: list[str])

    Example:
        can_run, blocking = await _validate_upstream_dependencies(seg, state)
        if not can_run:
            log.error(f"S{seg.num:02d} blocked by: {blocking}")
    """
    blocking = []

    # Check explicit dependencies from frontmatter
    for dep_num in seg.depends_on:
        dep_status_data = await state.get_segment(dep_num)
        if dep_status_data:
            dep_status = dep_status_data.status
        else:
            dep_status = "unknown"

        if dep_status != "pass":
            blocking.append(f"S{dep_num:02d}")

    if blocking:
        return False, blocking

    # All dependencies passed
    return True, []


async def _mark_dependents_skipped(
    seg_num: int,
    state: StateDB,
    all_segments: list[Segment],
    reason: str,
) -> list[int]:
    """Recursively mark all transitive dependents as skipped.

    Args:
        seg_num: Failed segment number
        state: State database
        all_segments: All segments (for dependency lookup)
        reason: Why root segment failed

    Returns:
        List of skipped segment numbers
    """
    seg = next((s for s in all_segments if s.num == seg_num), None)
    if not seg:
        return []

    skipped = []

    for dep_num in seg.dependents:
        dep_status_data = await state.get_segment(dep_num)
        if dep_status_data:
            dep_status = dep_status_data.status
        else:
            dep_status = "pending"

        # Only skip if segment is still pending
        if dep_status in ("pending", None):
            log.info(
                f"S{dep_num:02d} auto-skipped (transitive dependency on S{seg_num:02d})"
            )

            await state.set_status(dep_num, "skipped-dependency-failed")
            await state.log_event(
                "dependency_skip",
                f"S{dep_num:02d} skipped - transitive dependency failed: S{seg_num:02d} ({reason})",
                severity="info"
            )

            skipped.append(dep_num)

            # Recursively skip dependents of this segment
            transitive = await _mark_dependents_skipped(
                dep_num, state, all_segments, reason
            )
            skipped.extend(transitive)

    return skipped


class WaveRunner:
    """Manages parallel execution of segments within a wave."""

    def __init__(
        self,
        state: StateDB,
        config: OrchestrateConfig,
        notifier: Notifier,
        log_dir: Path,
        pool: WorktreePool | None = None,
        running_pids: dict[int, int] | None = None,
    ):
        self.state = state
        self.config = config
        self.notifier = notifier
        self.log_dir = log_dir
        self.pool = pool
        self.running_pids = running_pids or {}
        self.segment_executor = SegmentExecutor(config, state, notifier, log_dir)

    async def execute(
        self,
        wave_num: int,
        segments: list[Segment],
        shutting_down: asyncio.Event,
        all_segments: list[Segment] | None = None,
    ) -> list[tuple[int, str]]:
        """Execute all segments in a wave with bounded parallelism.

        Args:
            wave_num: Wave number (for logging)
            segments: Segments to execute in this wave
            shutting_down: Event signaling shutdown request
            all_segments: All segments in the plan (for dependency tracking)

        Returns:
            List of (segment_num, status) tuples
        """
        sem = asyncio.Semaphore(self.config.max_parallel)
        results: list[tuple[int, str]] = []

        async def _run_one_segment(seg: Segment) -> tuple[int, str]:
            """Execute a single segment with dependency validation and skip checks."""
            if shutting_down.is_set():
                return seg.num, "skipped"

            async with sem:
                if shutting_down.is_set():
                    return seg.num, "skipped"

                # Operator may have skipped this segment while it was queued
                current = await self.state.get_segment(seg.num)
                if current and current["status"] == "skipped":
                    return seg.num, "skipped"

                # Check if dependencies are satisfied
                can_run, blocking = await _validate_upstream_dependencies(seg, self.state)
                if not can_run:
                    log.warning(
                        f"S{seg.num:02d} skipped - blocked by dependencies: {blocking}"
                    )
                    await self.state.set_status(seg.num, "skipped-dependency-failed")
                    await self.state.log_event(
                        "dependency_skip",
                        f"S{seg.num:02d} skipped - upstream dependencies failed: {', '.join(blocking)}",
                        severity="info"
                    )
                    return seg.num, "skipped-dependency-failed"

                # Execute segment (with or without worktree)
                if self.pool and self.config.isolation_strategy == "worktree":
                    async with self.pool.acquire(seg.num) as wt:
                        status, final_status = await self.segment_executor.execute(
                            seg,
                            worktree=wt,
                            register_pid=lambda n, pid: self.running_pids.__setitem__(n, pid),
                            unregister_pid=lambda n: self.running_pids.pop(n, None),
                        )
                        return seg.num, final_status
                else:
                    # No worktree isolation
                    status, final_status = await self.segment_executor.execute(
                        seg,
                        worktree=None,
                        register_pid=lambda n, pid: self.running_pids.__setitem__(n, pid),
                        unregister_pid=lambda n: self.running_pids.pop(n, None),
                    )
                    return seg.num, final_status

        # Store segment numbers with tasks to preserve identity
        task_map = [
            (seg.num, asyncio.create_task(_run_one_segment(seg), name=f"S{seg.num:02d}"))
            for seg in segments
        ]
        tasks = [t for _, t in task_map]
        done = await asyncio.gather(*tasks, return_exceptions=True)

        for (seg_num, task), result in zip(task_map, done):
            if isinstance(result, Exception):
                log.error("Wave %d segment S%02d error: %s", wave_num, seg_num, result)
                results.append((seg_num, "error"))
            else:
                num, status = result
                results.append((num, status))

                # Treat merge conflicts as partial success
                if status == "pass-merge-conflict":
                    log.warning("S%02d completed but has merge conflicts - manual intervention needed", num)

        # Post-gather sweep: catch retries pressed after gather completed but before
        # the wave advances to the next wave. Re-run any segments reset to pending.
        retry_segs = []
        for seg in segments:
            refreshed = await self.state.get_segment(seg.num)
            if refreshed and refreshed["status"] == "pending":
                retry_segs.append(seg)

        if retry_segs:
            log.info("Wave %d: re-running %d operator-retried segment(s): %s",
                     wave_num, len(retry_segs), [s.num for s in retry_segs])
            retry_tasks = [asyncio.create_task(_run_one_segment(seg)) for seg in retry_segs]
            retry_done = await asyncio.gather(*retry_tasks, return_exceptions=True)

            # Replace old results for retried segments
            retried_map = {}
            for item in retry_done:
                if isinstance(item, tuple):
                    retried_map[item[0]] = item[1]

            results = [
                (n, retried_map.get(n, s)) for n, s in results
            ] + [(n, s) for n, s in retried_map.items() if n not in {r[0] for r in results}]

        # Mark transitive dependents as skipped for failed segments
        if all_segments:
            for seg_num, status in results:
                if status in ("failed", "blocked", "partial", "timeout", "skipped-dependency-failed"):
                    # Get summary for context
                    seg_data = await self.state.get_segment(seg_num)
                    summary = ""
                    if seg_data and seg_data.result_json:
                        try:
                            result = json.loads(seg_data.result_json)
                            summary = result.get("summary", "")[:200]
                        except (json.JSONDecodeError, KeyError):
                            pass

                    # Mark all transitive dependents as skipped
                    skipped = await _mark_dependents_skipped(
                        seg_num, self.state, all_segments, f"{status}: {summary}"
                    )

                    if skipped:
                        log.info(
                            f"S{seg_num:02d} failure caused {len(skipped)} segments to be skipped: "
                            f"{[f'S{n:02d}' for n in skipped]}"
                        )

        return results
