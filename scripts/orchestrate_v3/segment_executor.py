"""SegmentExecutor: Handles retry logic and execution for individual segments."""

from __future__ import annotations

import asyncio
import logging
from pathlib import Path
from typing import TYPE_CHECKING, Callable

if TYPE_CHECKING:
    from .config import OrchestrateConfig
    from .notify import Notifier
    from .planner import Segment
    from .state import StateDB
    from .worktree_pool import Worktree

from .runner import CircuitBreaker, run_segment

log = logging.getLogger("orchestrate")


async def _merge_worktree_changes(wt: Worktree, seg: Segment) -> bool:
    """Merge successful segment changes from worktree branch back to HEAD.

    Implements three-tier merge strategy:
    1. Try direct merge (fast path for no conflicts)
    2. If conflict: rebase worktree on HEAD, retry merge
    3. If still conflict: mark for manual resolution

    Returns True on clean merge, False on conflict.
    Works in both named branch and detached HEAD states.
    """
    try:
        # Get current branch name for logging (may be None in detached HEAD)
        proc = await asyncio.create_subprocess_exec(
            "git", "symbolic-ref", "--short", "HEAD",
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, stderr = await proc.communicate()
        current_branch = stdout.decode().strip() if proc.returncode == 0 else None

        if current_branch:
            log.info("Merging S%02d from %s to branch %s", seg.num, wt.branch, current_branch)
        else:
            log.info("Merging S%02d from %s to detached HEAD", seg.num, wt.branch)

        # Attempt 1: Direct merge
        proc = await asyncio.create_subprocess_exec(
            "git", "merge", "--no-ff", "-m",
            f"Merge segment S{seg.num:02d}: {seg.title}",
            wt.branch,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, stderr = await proc.communicate()

        if proc.returncode == 0:
            # Direct merge succeeded
            target = current_branch if current_branch else "detached HEAD"
            log.info("Successfully merged S%02d changes from %s to %s", seg.num, wt.branch, target)
            return True

        # Direct merge failed - try rebase strategy
        log.info(
            f"S{seg.num:02d} merge conflict detected, "
            f"attempting rebase on latest HEAD..."
        )

        # Abort the failed merge first
        await asyncio.create_subprocess_exec(
            "git", "merge", "--abort",
            stdout=asyncio.subprocess.DEVNULL,
            stderr=asyncio.subprocess.DEVNULL,
        )

        # Rebase worktree branch on current HEAD
        rebase_ok = await _rebase_worktree_on_head(wt, seg)
        if not rebase_ok:
            log.error(
                f"S{seg.num:02d} rebase failed - "
                f"conflicts require manual resolution"
            )
            return False

        # Attempt 2: Retry merge after rebase
        proc = await asyncio.create_subprocess_exec(
            "git", "merge", "--no-ff", "-m",
            f"Merge segment S{seg.num:02d}: {seg.title}",
            wt.branch,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE,
        )
        stdout, stderr = await proc.communicate()

        if proc.returncode == 0:
            log.info(f"S{seg.num:02d} merged successfully after rebase")
            return True

        # Both strategies failed - abort and mark for manual resolution
        await asyncio.create_subprocess_exec(
            "git", "merge", "--abort",
            stdout=asyncio.subprocess.DEVNULL,
            stderr=asyncio.subprocess.DEVNULL,
        )

        log.error(
            f"S{seg.num:02d} merge conflict persists after rebase - "
            f"manual resolution required"
        )
        return False

    except Exception as e:
        log.exception("Exception during merge for S%02d: %s", seg.num, e)
        return False


async def _rebase_worktree_on_head(wt: Worktree, seg: Segment) -> bool:
    """Rebase worktree branch on current HEAD.

    This replays worktree commits on top of latest HEAD,
    resolving many conflicts automatically via three-way merge.

    Args:
        wt: Worktree to rebase
        seg: Segment metadata

    Returns:
        True if rebase succeeded, False if conflicts remain
    """
    # Get current HEAD commit
    proc = await asyncio.create_subprocess_exec(
        "git", "rev-parse", "HEAD",
        stdout=asyncio.subprocess.PIPE,
    )
    head_sha, _ = await proc.communicate()
    head_sha = head_sha.decode().strip()

    log.debug(f"Rebasing {wt.branch} on {head_sha[:8]}")

    # In worktree: git rebase HEAD
    proc = await asyncio.create_subprocess_exec(
        "git", "-C", str(wt.path),
        "rebase", head_sha,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
    )
    stdout, stderr = await proc.communicate()

    if proc.returncode != 0:
        # Rebase failed - abort to clean state
        log.debug(f"Rebase failed for S{seg.num:02d}, aborting: {stderr.decode()}")

        await asyncio.create_subprocess_exec(
            "git", "-C", str(wt.path),
            "rebase", "--abort",
            stdout=asyncio.subprocess.DEVNULL,
            stderr=asyncio.subprocess.DEVNULL,
        )
        return False

    log.debug(f"Rebase succeeded for {wt.branch}")
    return True


class SegmentExecutor:
    """Executes a single segment with retry logic and circuit breaker."""

    def __init__(
        self,
        config: OrchestrateConfig,
        state: StateDB,
        notifier: Notifier,
        log_dir: Path,
    ):
        self.config = config
        self.state = state
        self.notifier = notifier
        self.log_dir = log_dir

    async def execute(
        self,
        seg: Segment,
        worktree: Worktree | None = None,
        register_pid: Callable[[int, int], None] | None = None,
        unregister_pid: Callable[[int], None] | None = None,
    ) -> tuple[str, str]:
        """Execute single segment with retry logic.

        Args:
            seg: Segment to execute
            worktree: Optional worktree for isolated execution
            register_pid: Optional callback to register running PID
            unregister_pid: Optional callback to unregister PID

        Returns:
            Tuple of (status, final_status) where final_status may include
            merge status like "merged" or "pass-merge-conflict"
        """
        cwd = worktree.path if worktree else None

        while True:  # Outer loop: re-enters when operator hits Retry mid-wave
            attempts = 0
            circuit = CircuitBreaker()  # Create circuit breaker for this segment
            status = "failed"
            summary = ""

            while attempts <= self.config.max_retries:
                attempts = await self.state.increment_attempts(seg.num)
                status, summary = await run_segment(
                    seg,
                    self.config,
                    self.state,
                    self.log_dir,
                    notifier=self.notifier,
                    attempt_num=attempts,
                    register_pid=register_pid,
                    unregister_pid=unregister_pid,
                    cwd=cwd,
                )

                if status in ("pass", "timeout"):
                    break

                # Check if status is retryable per policy
                if not self.config.retry_policy.should_retry(status):
                    log.info("S%02d status '%s' not retryable per policy", seg.num, status)
                    break

                # Check circuit breaker for permanent failure patterns
                should_retry_cb, circuit_reason = circuit.should_retry(summary)
                if not should_retry_cb:
                    log.warning("S%02d circuit breaker tripped: %s", seg.num, circuit_reason)
                    await self.state.log_event(
                        "circuit_breaker_trip",
                        f"S{seg.num:02d} - {circuit_reason}",
                        severity="warning"
                    )
                    break

                if attempts > self.config.max_retries:
                    break

                # For PARTIAL and UNKNOWN status, retry immediately without delay
                # PARTIAL = work in progress, UNKNOWN = couldn't parse status (likely completed but format issue)
                # For other retryable statuses (failed, timeout), use exponential backoff
                if status in ("partial", "unknown"):
                    log.info("S%02d %s status - continuing immediately (attempt %d/%d)",
                             seg.num, status.upper(), attempts + 1, self.config.max_retries)
                    await self.state.log_event("segment_continue",
                                               f"S{seg.num:02d} continuing from {status} (attempt {attempts + 1})")
                else:
                    delay = self.config.retry_policy.get_delay(attempts - 1)
                    log.info("S%02d retrying in %ds (attempt %d/%d)",
                             seg.num, delay, attempts + 1, self.config.max_retries)
                    await self.state.log_event("segment_retry",
                                               f"S{seg.num:02d} attempt {attempts + 1} after {delay}s")
                    await asyncio.sleep(delay)

            # Check if operator reset us to pending while we were running or
            # immediately after — if so, re-run without requiring an orchestrator restart.
            refreshed = await self.state.get_segment(seg.num)
            if refreshed and refreshed["status"] == "pending":
                log.info("S%02d operator retry detected, re-running in-wave", seg.num)
                await self.state.log_event("segment_retry", f"S{seg.num:02d} operator retry (in-wave)")
                continue
            break

        # Handle worktree merge if applicable
        if worktree and status == "pass" and self.config.isolation_strategy == "worktree":
            merge_ok = await _merge_worktree_changes(worktree, seg)
            if not merge_ok:
                log.warning("S%02d passed but merge failed - manual intervention needed", seg.num)
                return status, "pass-merge-conflict"
            # Mark as merged in database
            await self.state.mark_merged(seg.num)
            return status, "merged"

        return status, status
