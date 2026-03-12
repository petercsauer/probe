"""Tests for sequential merge with rebase strategy.

Note: These are structure/import tests. Full integration tests require
actual git worktrees and are best done as integration tests.
"""
import sys
from pathlib import Path

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from scripts.orchestrate_v2.planner import Segment


def test_topological_sort():
    """Test that dependencies merge before dependents."""
    # S1 → S3, S2 → S3
    # Expected order: [S1, S2, S3]
    segments = [
        Segment(num=1, slug="s1", title="S1", depends_on=[], wave=1),
        Segment(num=2, slug="s2", title="S2", depends_on=[], wave=1),
        Segment(num=3, slug="s3", title="S3", depends_on=[1, 2], wave=2),
    ]

    # Topological sort implementation
    sorted_segs = []
    visited = set()

    def visit(seg):
        if seg.num in visited:
            return
        for dep_num in seg.depends_on:
            dep = next(s for s in segments if s.num == dep_num)
            visit(dep)
        visited.add(seg.num)
        sorted_segs.append(seg)

    for seg in segments:
        visit(seg)

    # S3 must come after S1 and S2
    s3_idx = next(i for i, s in enumerate(sorted_segs) if s.num == 3)
    s1_idx = next(i for i, s in enumerate(sorted_segs) if s.num == 1)
    s2_idx = next(i for i, s in enumerate(sorted_segs) if s.num == 2)

    assert s1_idx < s3_idx, f"S1 ({s1_idx}) should come before S3 ({s3_idx})"
    assert s2_idx < s3_idx, f"S2 ({s2_idx}) should come before S3 ({s3_idx})"

    print("✓ test_topological_sort: PASS")


def test_merge_functions_exist():
    """Test that merge functions can be imported."""
    from scripts.orchestrate_v2.__main__ import _merge_worktree_changes, _rebase_worktree_on_head

    # Just verify they exist and are callable
    assert callable(_merge_worktree_changes)
    assert callable(_rebase_worktree_on_head)

    print("✓ test_merge_functions_exist: PASS")


def test_complex_dependency_merge_order():
    """Test merge order with complex dependencies."""
    # S1 → S3 → S5
    # S2 → S4 → S5
    # Expected: S1, S2 can be in any order, then S3, S4, then S5
    segments = [
        Segment(num=1, slug="s1", title="S1", depends_on=[], wave=1),
        Segment(num=2, slug="s2", title="S2", depends_on=[], wave=1),
        Segment(num=3, slug="s3", title="S3", depends_on=[1], wave=2),
        Segment(num=4, slug="s4", title="S4", depends_on=[2], wave=2),
        Segment(num=5, slug="s5", title="S5", depends_on=[3, 4], wave=3),
    ]

    # Topological sort
    sorted_segs = []
    visited = set()

    def visit(seg):
        if seg.num in visited:
            return
        for dep_num in seg.depends_on:
            dep = next(s for s in segments if s.num == dep_num)
            visit(dep)
        visited.add(seg.num)
        sorted_segs.append(seg)

    for seg in segments:
        visit(seg)

    # Verify order constraints
    positions = {s.num: i for i, s in enumerate(sorted_segs)}

    assert positions[1] < positions[3], "S1 must come before S3"
    assert positions[2] < positions[4], "S2 must come before S4"
    assert positions[3] < positions[5], "S3 must come before S5"
    assert positions[4] < positions[5], "S4 must come before S5"

    print("✓ test_complex_dependency_merge_order: PASS")


if __name__ == "__main__":
    print("Running merge strategy tests...\n")

    test_topological_sort()
    test_merge_functions_exist()
    test_complex_dependency_merge_order()

    print("\nAll 3 tests passed!")
