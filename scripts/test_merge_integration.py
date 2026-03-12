#!/usr/bin/env python3
"""Test script for merge integration."""

import asyncio
import sys
from pathlib import Path

# Add scripts to path
sys.path.insert(0, str(Path(__file__).parent))

from orchestrate_v2.__main__ import _merge_worktree_changes
from orchestrate_v2.worktree_pool import WorktreePool, Worktree
from orchestrate_v2.planner import Segment


async def test_merge_clean():
    """Test that clean merge works."""
    print("Testing clean merge scenario...")

    import subprocess

    # Stash any uncommitted changes to ensure clean test environment
    stash_result = subprocess.run(
        ["git", "stash", "push", "-u", "-m", "test_merge_integration stash"],
        capture_output=True,
        text=True
    )
    stashed = "No local changes to save" not in stash_result.stdout

    try:
        # Create a temporary worktree for testing
        # Use HEAD to work with current state (whether branch or detached)
        pool = WorktreePool(
            repo_root=Path.cwd(),
            pool_size=1,
            target_branch="HEAD"
        )

        await pool.create()
        print("[OK] Created test pool")

        # Acquire worktree and make a simple change
        async with pool.acquire(seg_num=99) as wt:
            print(f"[OK] Acquired worktree: {wt.path}")

            # Create a test file in the worktree
            test_file = wt.path / "test_merge_file.txt"
            test_file.write_text("Test content for merge\n")

            # Stage and commit the change
            subprocess.run(
                ["git", "add", "test_merge_file.txt"],
                cwd=wt.path,
                check=True,
                capture_output=True
            )
            subprocess.run(
                ["git", "commit", "-m", "Test commit for merge"],
                cwd=wt.path,
                check=True,
                capture_output=True
            )
            print("[OK] Created test commit in worktree")

            # Create a fake segment
            seg = Segment(
                num=99,
                slug="test-merge",
                title="Test Merge",
                file_path=Path("test.md")
            )

            # Test merge
            print("Testing merge...")
            result = await _merge_worktree_changes(wt, seg)

            if result:
                print("[OK] Merge successful")
            else:
                print("[X] Merge failed")
                await pool.cleanup()
                return False

        # Clean up the test file
        test_file_main = Path.cwd() / "test_merge_file.txt"
        if test_file_main.exists():
            test_file_main.unlink()
            subprocess.run(
                ["git", "add", "test_merge_file.txt"],
                cwd=Path.cwd(),
                check=True,
                capture_output=True
            )
            subprocess.run(
                ["git", "commit", "-m", "Remove test file"],
                cwd=Path.cwd(),
                check=True,
                capture_output=True
            )
            print("[OK] Cleaned up test file")

        # Cleanup pool
        await pool.cleanup()
        print("[OK] Pool cleaned up")

        print("\nPASS Merge integration test passed!")
        return True

    finally:
        # Restore stashed changes if any
        if stashed:
            subprocess.run(
                ["git", "stash", "pop"],
                capture_output=True
            )


async def test_detached_head_merge():
    """Test that merge works when repository is in detached HEAD state."""
    print("Testing detached HEAD merge scenario...")

    import subprocess

    # Stash any uncommitted changes to ensure clean test environment
    stash_result = subprocess.run(
        ["git", "stash", "push", "-u", "-m", "test_detached_head_merge stash"],
        capture_output=True,
        text=True
    )
    stashed = "No local changes to save" not in stash_result.stdout

    # Save current HEAD
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
        check=True
    )
    original_head = result.stdout.strip()

    # Get current branch if any
    result = subprocess.run(
        ["git", "symbolic-ref", "--short", "HEAD"],
        capture_output=True,
        text=True
    )
    original_branch = result.stdout.strip() if result.returncode == 0 else None

    try:
        # Enter detached HEAD state
        subprocess.run(
            ["git", "checkout", "--detach", "HEAD"],
            check=True,
            capture_output=True
        )
        print("[OK] Entered detached HEAD state")

        # Verify we're in detached HEAD
        result = subprocess.run(
            ["git", "symbolic-ref", "--short", "HEAD"],
            capture_output=True,
            text=True
        )
        if result.returncode == 0:
            print("[X] Failed to enter detached HEAD state")
            return False
        print("[OK] Confirmed detached HEAD state")

        # Create a temporary worktree for testing
        pool = WorktreePool(
            repo_root=Path.cwd(),
            pool_size=1,
            target_branch="HEAD"  # Use HEAD instead of main
        )

        await pool.create()
        print("[OK] Created test pool")

        # Acquire worktree and make a simple change
        async with pool.acquire(seg_num=97) as wt:
            print(f"[OK] Acquired worktree: {wt.path}")

            # Create a test file in the worktree
            test_file = wt.path / "test_detached_merge_file.txt"
            test_file.write_text("Test content for detached HEAD merge\n")

            # Stage and commit the change
            subprocess.run(
                ["git", "add", "test_detached_merge_file.txt"],
                cwd=wt.path,
                check=True,
                capture_output=True
            )
            subprocess.run(
                ["git", "commit", "-m", "Test commit for detached HEAD merge"],
                cwd=wt.path,
                check=True,
                capture_output=True
            )
            print("[OK] Created test commit in worktree")

            # Create a fake segment
            seg = Segment(
                num=97,
                slug="test-detached-merge",
                title="Test Detached HEAD Merge",
                file_path=Path("test.md")
            )

            # Test merge - should succeed even in detached HEAD
            print("Testing merge in detached HEAD...")
            result = await _merge_worktree_changes(wt, seg)

            if result:
                print("[OK] Merge successful in detached HEAD")
            else:
                print("[X] Merge failed in detached HEAD")
                await pool.cleanup()
                return False

        # Verify the file was merged to main repo
        test_file_main = Path.cwd() / "test_detached_merge_file.txt"
        if not test_file_main.exists():
            print("[X] Merged file not found in main repo")
            await pool.cleanup()
            return False
        print("[OK] Verified file was merged")

        # Clean up the test file
        test_file_main.unlink()
        subprocess.run(
            ["git", "add", "test_detached_merge_file.txt"],
            cwd=Path.cwd(),
            check=True,
            capture_output=True
        )
        subprocess.run(
            ["git", "commit", "-m", "Remove test file"],
            cwd=Path.cwd(),
            check=True,
            capture_output=True
        )
        print("[OK] Cleaned up test file")

        # Cleanup pool
        await pool.cleanup()
        print("[OK] Pool cleaned up")

        print("\nPASS Detached HEAD merge test passed!")
        return True

    finally:
        # Restore original HEAD state
        if original_branch:
            subprocess.run(
                ["git", "checkout", original_branch],
                capture_output=True
            )
        else:
            subprocess.run(
                ["git", "checkout", original_head],
                capture_output=True
            )

        # Restore stashed changes if any
        if stashed:
            subprocess.run(
                ["git", "stash", "pop"],
                capture_output=True
            )


if __name__ == "__main__":
    # Run both tests
    import sys

    result1 = asyncio.run(test_merge_clean())
    if not result1:
        sys.exit(1)

    result2 = asyncio.run(test_detached_head_merge())
    sys.exit(0 if result2 else 1)
