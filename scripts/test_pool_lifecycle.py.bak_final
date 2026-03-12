#!/usr/bin/env python3
"""Test script for worktree pool lifecycle integration."""

import asyncio
import sys
from pathlib import Path

# Add scripts to path
sys.path.insert(0, str(Path(__file__).parent))

from orchestrate_v2.worktree_pool import WorktreePool


async def test_pool_lifecycle():
    """Test that pool can be created and cleaned up."""
    print("Testing worktree pool lifecycle...")

    # Create pool
    pool = WorktreePool(
        repo_root=Path.cwd(),
        pool_size=2,
        target_branch="main"
    )

    print("Creating pool...")
    await pool.create()
    print(f"✓ Created pool with {len(pool._worktrees)} worktrees")

    # Test acquire/release
    print("Testing acquire/release...")
    async with pool.acquire(seg_num=1) as wt:
        print(f"✓ Acquired worktree: {wt.path} (branch: {wt.branch})")
        assert wt.path.exists()
        assert wt.current_segment == 1

    print("✓ Released worktree")

    # Cleanup
    print("Cleaning up pool...")
    await pool.cleanup()
    print("✓ Pool cleaned up")

    print("\n✅ All tests passed!")


if __name__ == "__main__":
    asyncio.run(test_pool_lifecycle())
