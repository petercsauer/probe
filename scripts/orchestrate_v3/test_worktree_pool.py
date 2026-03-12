"""Tests for WorktreePool functionality."""

import asyncio
import subprocess
from pathlib import Path

import pytest

from worktree_pool import WorktreePool


@pytest.fixture
def repo_root() -> Path:
    """Get repository root directory.

    Returns:
        Path to repository root containing .git directory
    """
    root = Path.cwd()
    if not (root / ".git").exists():
        # We're in a subdirectory, navigate up
        root = Path(__file__).parent.parent.parent
    return root


@pytest.fixture
async def worktree_pool(repo_root: Path):
    """Create and cleanup a WorktreePool for testing.

    Args:
        repo_root: Repository root directory

    Yields:
        Initialized WorktreePool instance
    """
    pool = WorktreePool(repo_root=repo_root, pool_size=3, target_branch="main")
    await pool.create()
    yield pool
    await pool.cleanup()


async def test_pool_creation(worktree_pool: WorktreePool, repo_root: Path):
    """Test that worktree pool is created successfully."""
    # Verify worktrees exist
    result = subprocess.run(
        ["git", "worktree", "list"],
        cwd=repo_root,
        capture_output=True,
        text=True
    )
    # Should have pool worktrees in the list
    assert ".claude/worktrees" in result.stdout


async def test_acquire_release(worktree_pool: WorktreePool):
    """Test acquiring and releasing a worktree."""
    async with worktree_pool.acquire(seg_num=1) as wt:
        assert wt.pool_id is not None
        assert wt.path.exists()
        assert wt.branch is not None
        assert wt.current_segment == 1

        # Verify the worktree is clean
        result = subprocess.run(
            ["git", "status", "--short"],
            cwd=wt.path,
            capture_output=True,
            text=True
        )
        assert result.stdout.strip() == "", "Worktree should be clean"


async def test_concurrent_acquisition(worktree_pool: WorktreePool):
    """Test that multiple segments can acquire worktrees concurrently."""
    async def acquire_test(seg_num: int) -> int:
        async with worktree_pool.acquire(seg_num=seg_num) as wt:
            await asyncio.sleep(0.1)
            return wt.pool_id

    results = await asyncio.gather(
        acquire_test(1),
        acquire_test(2),
        acquire_test(3)
    )

    # All three should have acquired different pool IDs
    assert len(results) == 3
    assert len(set(results)) == 3, "Each segment should get a different worktree"


async def test_cleanup(repo_root: Path):
    """Test that cleanup removes all pool worktrees."""
    pool = WorktreePool(repo_root=repo_root, pool_size=2, target_branch="main")
    await pool.create()

    # Get the specific worktree paths this pool created
    pool_paths = {str(wt.path) for wt in pool._worktrees}

    # Verify worktrees exist before cleanup
    result = subprocess.run(
        ["git", "worktree", "list"],
        cwd=repo_root,
        capture_output=True,
        text=True
    )
    for path in pool_paths:
        assert path in result.stdout, f"Expected {path} to exist before cleanup"

    # Cleanup
    await pool.cleanup()

    # Verify worktrees removed
    result = subprocess.run(
        ["git", "worktree", "list"],
        cwd=repo_root,
        capture_output=True,
        text=True
    )
    # Check that our specific pool worktrees are gone
    for path in pool_paths:
        assert path not in result.stdout, f"Expected {path} to be removed after cleanup"
