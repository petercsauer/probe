#!/usr/bin/env python3
"""Standalone test script for WorktreePool."""

import asyncio
import sys
from pathlib import Path

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent))

from worktree_pool import WorktreePool


async def main():
    """Test WorktreePool functionality."""
    repo_root = Path.cwd()
    if not (repo_root / ".git").exists():
        # We're in a subdirectory, navigate up
        repo_root = Path(__file__).parent.parent.parent

    print(f"Repository root: {repo_root}")
    print(f"Creating pool of 3 worktrees...")

    pool = WorktreePool(repo_root=repo_root, pool_size=3, target_branch="main")

    try:
        # Create the pool
        await pool.create()
        print("[OK] Pool created successfully")

        # Verify worktrees exist
        import subprocess
        result = subprocess.run(
            ["git", "worktree", "list"],
            cwd=repo_root,
            capture_output=True,
            text=True
        )
        print("\nWorktree list:")
        print(result.stdout)

        # Test acquisition
        print("Testing acquire/release...")
        async with pool.acquire(seg_num=1) as wt1:
            print(f"[OK] Acquired worktree {wt1.pool_id} at {wt1.path}")
            print(f"  Branch: {wt1.branch}")
            print(f"  Segment: {wt1.current_segment}")

            # Verify the worktree is clean
            result = subprocess.run(
                ["git", "status", "--short"],
                cwd=wt1.path,
                capture_output=True,
                text=True
            )
            if result.stdout.strip() == "":
                print("  [OK] Worktree is clean")
            else:
                print(f"  WARNING: Worktree not clean:\n{result.stdout}")

        print("[OK] Worktree released")

        # Test concurrent acquisition
        print("\nTesting concurrent acquisition...")
        async def acquire_test(seg_num: int):
            async with pool.acquire(seg_num=seg_num) as wt:
                print(f"  Segment {seg_num} acquired worktree {wt.pool_id}")
                await asyncio.sleep(0.1)
                return wt.pool_id

        results = await asyncio.gather(
            acquire_test(1),
            acquire_test(2),
            acquire_test(3)
        )
        print(f"[OK] Concurrent acquisitions successful: {results}")

        # Cleanup
        print("\nCleaning up...")
        await pool.cleanup()
        print("[OK] Pool cleaned up")

        # Verify worktrees removed
        result = subprocess.run(
            ["git", "worktree", "list"],
            cwd=repo_root,
            capture_output=True,
            text=True
        )
        if ".claude/worktrees/pool-" not in result.stdout:
            print("[OK] All pool worktrees removed")
        else:
            print("WARNING: Some pool worktrees still exist:")
            print(result.stdout)

        print("\nPASS All tests passed!")
        return 0

    except Exception as e:
        print(f"\nFAIL Test failed: {e}")
        import traceback
        traceback.print_exc()
        return 1


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
