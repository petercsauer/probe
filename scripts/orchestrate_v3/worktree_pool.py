"""Worktree pool manager for segment isolation.

Manages a fixed pool of git worktrees that can be acquired and released
for concurrent segment execution with complete isolation.
"""

from __future__ import annotations

import asyncio
import subprocess
from contextlib import asynccontextmanager
from dataclasses import dataclass, field
from pathlib import Path


@dataclass
class Worktree:
    """Represents a single worktree in the pool."""
    pool_id: int
    path: Path
    branch: str
    current_segment: int | None = None


class WorktreePool:
    """Manages a pool of git worktrees for parallel segment execution."""

    def __init__(
        self,
        repo_root: Path,
        pool_size: int,
        target_branch: str = "main"
    ):
        """Initialize worktree pool.

        Args:
            repo_root: Root directory of the git repository
            pool_size: Number of worktrees to create in the pool
            target_branch: Branch to reset worktrees to (default: main)
        """
        self._repo_root = repo_root
        self._pool_size = pool_size
        self._target_branch = target_branch
        self._worktrees: list[Worktree] = []
        self._queue: asyncio.Queue[Worktree] = asyncio.Queue()

    async def create(self) -> None:
        """Create pool of worktrees, reusing existing ones if possible.

        This method:
        1. Prunes stale worktree references
        2. Creates or reuses worktrees in .claude/worktrees/pool-{id:02d}
        3. Initializes the acquisition queue
        """
        worktrees_dir = self._repo_root / ".claude" / "worktrees"
        worktrees_dir.mkdir(parents=True, exist_ok=True)

        # Prune stale worktree references
        await self._run_git(["worktree", "prune"])

        # Get list of existing worktrees
        existing_worktrees = await self._get_existing_worktrees()

        for i in range(self._pool_size):
            pool_id = i
            wt_path = worktrees_dir / f"pool-{pool_id:02d}"
            wt_branch = f"wt/pool-{pool_id:02d}"

            # Check if worktree already exists
            if str(wt_path) in existing_worktrees:
                # Reuse existing worktree
                worktree = Worktree(
                    pool_id=pool_id,
                    path=wt_path,
                    branch=wt_branch
                )
            else:
                # Check if branch exists
                branch_exists = await self._branch_exists(wt_branch)

                # Create new worktree
                if branch_exists:
                    # Branch exists but worktree doesn't - reuse branch
                    await self._run_git([
                        "worktree", "add",
                        str(wt_path),
                        wt_branch
                    ])
                else:
                    # Create new branch and worktree
                    await self._run_git([
                        "worktree", "add",
                        str(wt_path),
                        "-b", wt_branch,
                        "HEAD"
                    ])

                worktree = Worktree(
                    pool_id=pool_id,
                    path=wt_path,
                    branch=wt_branch
                )

            self._worktrees.append(worktree)
            self._queue.put_nowait(worktree)

    @asynccontextmanager
    async def acquire(self, seg_num: int = 0):
        """Acquire a worktree from the pool for a segment.

        Blocks if all worktrees are currently in use. The worktree is
        reset to the target branch before being yielded.

        Args:
            seg_num: Segment number for tracking (default: 0)

        Yields:
            Worktree: An available worktree
        """
        wt = await self._queue.get()
        wt.current_segment = seg_num

        try:
            # Reset worktree to target branch
            await self._run_git(
                ["reset", "--hard", self._target_branch],
                cwd=wt.path
            )
            await self._run_git(
                ["clean", "-fdx"],
                cwd=wt.path
            )
            yield wt
        finally:
            wt.current_segment = None
            self._queue.put_nowait(wt)

    async def cleanup(self) -> None:
        """Remove all worktrees from the pool.

        This should be called when the pool is no longer needed.
        """
        for wt in self._worktrees:
            try:
                await self._run_git([
                    "worktree", "remove",
                    "--force",
                    str(wt.path)
                ])
            except subprocess.CalledProcessError:
                # Best effort cleanup - continue even if removal fails
                pass

        self._worktrees.clear()

    async def _get_existing_worktrees(self) -> set[str]:
        """Get set of existing worktree paths."""
        try:
            result = await self._run_git(["worktree", "list", "--porcelain"])
            worktree_paths = set()
            for line in result.stdout.splitlines():
                if line.startswith("worktree "):
                    worktree_paths.add(line[9:])  # Remove "worktree " prefix
            return worktree_paths
        except subprocess.CalledProcessError:
            return set()

    async def _branch_exists(self, branch_name: str) -> bool:
        """Check if a branch exists."""
        try:
            await self._run_git(["rev-parse", "--verify", branch_name])
            return True
        except subprocess.CalledProcessError:
            return False

    async def _run_git(
        self,
        args: list[str],
        cwd: Path | None = None
    ) -> subprocess.CompletedProcess:
        """Run a git command asynchronously.

        Args:
            args: Git command arguments (without 'git' prefix)
            cwd: Working directory (default: repo_root)

        Returns:
            CompletedProcess with stdout/stderr captured
        """
        cmd = ["git"] + args
        work_dir = cwd if cwd else self._repo_root

        proc = await asyncio.create_subprocess_exec(
            *cmd,
            cwd=work_dir,
            stdout=asyncio.subprocess.PIPE,
            stderr=asyncio.subprocess.PIPE
        )

        stdout, stderr = await proc.communicate()

        if proc.returncode != 0:
            raise subprocess.CalledProcessError(
                proc.returncode,
                cmd,
                stdout,
                stderr
            )

        return subprocess.CompletedProcess(
            args=cmd,
            returncode=proc.returncode,
            stdout=stdout.decode(),
            stderr=stderr.decode()
        )
