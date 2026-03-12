"""Tests for dependency graph and transitive failure propagation."""
import sys
from pathlib import Path

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from scripts.orchestrate_v2.planner import Segment, _compute_transitive_dependents, _assign_waves


def test_compute_dependents():
    """Test reverse edge computation."""
    # S1 → S3, S2 → S3
    segments = [
        Segment(num=1, slug="s1", title="S1", depends_on=[], wave=0),
        Segment(num=2, slug="s2", title="S2", depends_on=[], wave=0),
        Segment(num=3, slug="s3", title="S3", depends_on=[1, 2], wave=0),
    ]

    _compute_transitive_dependents(segments)

    assert segments[0].dependents == [3], f"Expected [3], got {segments[0].dependents}"
    assert segments[1].dependents == [3], f"Expected [3], got {segments[1].dependents}"
    assert segments[2].dependents == [], f"Expected [], got {segments[2].dependents}"
    print("[OK] test_compute_dependents: PASS")


def test_circular_dependency_detection():
    """Test that circular dependencies are detected."""
    # S1 → S2 → S3 → S1 (circular)
    segments = [
        Segment(num=1, slug="s1", title="S1", depends_on=[3], wave=0),
        Segment(num=2, slug="s2", title="S2", depends_on=[1], wave=0),
        Segment(num=3, slug="s3", title="S3", depends_on=[2], wave=0),
    ]

    try:
        _assign_waves(segments)
        assert False, "Should have raised ValueError for circular dependency"
    except ValueError as e:
        assert "Circular dependency" in str(e)
        print("[OK] test_circular_dependency_detection: PASS")


def test_topological_sort():
    """Test wave assignment respects dependency order."""
    # S1 (no deps), S2 (no deps), S3 (depends on 1, 2)
    segments = [
        Segment(num=1, slug="s1", title="S1", depends_on=[], wave=0),
        Segment(num=2, slug="s2", title="S2", depends_on=[], wave=0),
        Segment(num=3, slug="s3", title="S3", depends_on=[1, 2], wave=0),
    ]

    _assign_waves(segments)

    # S1 and S2 should be in wave 1, S3 should be in wave 2
    assert segments[0].wave == 1, f"S1 should be wave 1, got {segments[0].wave}"
    assert segments[1].wave == 1, f"S2 should be wave 1, got {segments[1].wave}"
    assert segments[2].wave == 2, f"S3 should be wave 2, got {segments[2].wave}"
    print("[OK] test_topological_sort: PASS")


def test_complex_dependency_chain():
    """Test complex dependency chain."""
    # S1 → S3 → S5
    # S2 → S4 → S5
    segments = [
        Segment(num=1, slug="s1", title="S1", depends_on=[], wave=0),
        Segment(num=2, slug="s2", title="S2", depends_on=[], wave=0),
        Segment(num=3, slug="s3", title="S3", depends_on=[1], wave=0),
        Segment(num=4, slug="s4", title="S4", depends_on=[2], wave=0),
        Segment(num=5, slug="s5", title="S5", depends_on=[3, 4], wave=0),
    ]

    _assign_waves(segments)
    _compute_transitive_dependents(segments)

    # Wave assignment
    assert segments[0].wave == 1, f"S1 wave {segments[0].wave}"
    assert segments[1].wave == 1, f"S2 wave {segments[1].wave}"
    assert segments[2].wave == 2, f"S3 wave {segments[2].wave}"
    assert segments[3].wave == 2, f"S4 wave {segments[3].wave}"
    assert segments[4].wave == 3, f"S5 wave {segments[4].wave}"

    # Dependents
    assert segments[0].dependents == [3], f"S1 dependents {segments[0].dependents}"
    assert segments[1].dependents == [4], f"S2 dependents {segments[1].dependents}"
    assert segments[2].dependents == [5], f"S3 dependents {segments[2].dependents}"
    assert segments[3].dependents == [5], f"S4 dependents {segments[3].dependents}"
    assert segments[4].dependents == [], f"S5 dependents {segments[4].dependents}"

    print("[OK] test_complex_dependency_chain: PASS")


if __name__ == "__main__":
    print("Running dependency graph tests...\n")

    test_compute_dependents()
    test_circular_dependency_detection()
    test_topological_sort()
    test_complex_dependency_chain()

    print("\nAll 4 tests passed!")
