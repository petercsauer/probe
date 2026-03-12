#!/usr/bin/env python3
"""Test merge conflict handling."""

import asyncio
import subprocess
import sys
from pathlib import Path

# Add scripts to path
sys.path.insert(0, str(Path(__file__).parent))

from orchestrate_v2.__main__ import _merge_worktree_changes
from orchestrate_v2.worktree_pool import WorktreePool
from orchestrate_v2.planner import Segment


async def test_conflict_abort():
    """Test that merge conflicts are handled gracefully."""
    print("Testing merge conflict handling...")

    # Save current HEAD and branch state
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
        check=True
    )
    original_head = result.stdout.strip()

    result = subprocess.run(
        ["git", "symbolic-ref", "--short", "HEAD"],
        capture_output=True,
        text=True
    )
    original_branch = result.stdout.strip() if result.returncode == 0 else None

    # Try to checkout main for cleaner state, but continue if not possible
    result = subprocess.run(
        ["git", "checkout", "main"],
        capture_output=True,
        text=True
    )
    if result.returncode == 0:
        original_branch = "main"
        print("[OK] Checked out main branch")
    else:
        print("[OK] Using current HEAD state")

    try:
        # Create a temporary worktree for testing
        # Get current commit SHA to reset worktrees to
        result = subprocess.run(
            ["git", "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            check=True
        )
        current_commit = result.stdout.strip()

        pool = WorktreePool(
            repo_root=Path.cwd(),
            pool_size=1,
            target_branch=current_commit
        )

        await pool.create()
        print("[OK] Created test pool")

        # Create a conflicting change in main
        # Use unique filename to avoid issues with previous test runs
        import uuid
        conflict_filename = f"test_conflict_{uuid.uuid4().hex[:8]}.txt"
        conflict_file = Path.cwd() / conflict_filename
        conflict_file.write_text("Main version\n")
        subprocess.run(
            ["git", "add", conflict_filename],
            check=True,
            capture_output=True
        )
        subprocess.run(
            ["git", "commit", "-m", "Add conflict file in main"],
            check=True,
            capture_output=True
        )
        print("[OK] Created conflicting commit in main")

        # Acquire worktree and make a conflicting change
        async with pool.acquire(seg_num=98) as wt:
            print(f"[OK] Acquired worktree: {wt.path}")

            # Create conflicting content
            test_file = wt.path / conflict_filename
            test_file.write_text("Worktree version\n")

            subprocess.run(
                ["git", "add", conflict_filename],
                cwd=wt.path,
                check=True,
                capture_output=True
            )
            subprocess.run(
                ["git", "commit", "-m", "Add conflicting change in worktree"],
                cwd=wt.path,
                check=True,
                capture_output=True
            )
            print("[OK] Created conflicting commit in worktree")

            # Create a fake segment
            seg = Segment(
                num=98,
                slug="test-conflict",
                title="Test Conflict",
                file_path=Path("test.md")
            )

            # Test merge - should fail due to conflict
            print("Testing merge with conflict...")
            result = await _merge_worktree_changes(wt, seg)

            if not result:
                print("[OK] Merge correctly returned False for conflict")
            else:
                print("[X] Merge should have failed due to conflict")
                await pool.cleanup()
                return False

        # Clean up
        subprocess.run(
            ["git", "reset", "--hard", "HEAD~1"],
            check=True,
            capture_output=True
        )
        conflict_file.unlink(missing_ok=True)
        print("[OK] Cleaned up test changes")

        await pool.cleanup()
        print("[OK] Pool cleaned up")

        print("\nPASS Merge conflict handling test passed!")
        return True

    finally:
        # Restore original branch/state
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


if __name__ == "__main__":
    result = asyncio.run(test_conflict_abort())
    sys.exit(0 if result else 1)
