"""Comprehensive tests for planner.py (frontmatter parsing, wave assignment, dependency resolution)."""

from pathlib import Path
import pytest

from .planner import (
    Segment,
    PlanMeta,
    _parse_frontmatter,
    _compute_transitive_dependents,
    _assign_waves,
    load_plan,
)


# =============================================================================
# _parse_frontmatter tests
# =============================================================================

def test_parse_frontmatter_valid_yaml(tmp_path):
    """Test parsing valid frontmatter with various data types."""
    md = tmp_path / "test.md"
    md.write_text("""---
segment: 5
title: "Test Segment"
depends_on: [1, 2, 3]
cycle_budget: 20
risk: 3
complexity: "High"
---
# Content
""")
    fm = _parse_frontmatter(md)
    assert fm["segment"] == 5
    assert fm["title"] == "Test Segment"
    assert fm["depends_on"] == [1, 2, 3]
    assert fm["cycle_budget"] == 20
    assert fm["risk"] == 3
    assert fm["complexity"] == "High"


def test_parse_frontmatter_empty_list(tmp_path):
    """Test parsing frontmatter with empty list."""
    md = tmp_path / "test.md"
    md.write_text("""---
segment: 1
depends_on: []
---
""")
    fm = _parse_frontmatter(md)
    assert fm["depends_on"] == []


def test_parse_frontmatter_string_list(tmp_path):
    """Test parsing frontmatter with list of strings."""
    md = tmp_path / "test.md"
    md.write_text("""---
tags: [high-priority, bug-fix, critical]
---
""")
    fm = _parse_frontmatter(md)
    assert fm["tags"] == ["high-priority", "bug-fix", "critical"]


def test_parse_frontmatter_single_quotes(tmp_path):
    """Test parsing frontmatter with single-quoted strings."""
    md = tmp_path / "test.md"
    md.write_text("""---
title: 'Single Quoted Title'
---
""")
    fm = _parse_frontmatter(md)
    assert fm["title"] == "Single Quoted Title"


def test_parse_frontmatter_double_quotes(tmp_path):
    """Test parsing frontmatter with double-quoted strings."""
    md = tmp_path / "test.md"
    md.write_text("""---
title: "Double Quoted Title"
---
""")
    fm = _parse_frontmatter(md)
    assert fm["title"] == "Double Quoted Title"


def test_parse_frontmatter_with_comments(tmp_path):
    """Test parsing frontmatter with comments (should be ignored)."""
    md = tmp_path / "test.md"
    md.write_text("""---
segment: 5
# This is a comment
title: "Test"
---
""")
    fm = _parse_frontmatter(md)
    assert fm["segment"] == 5
    assert fm["title"] == "Test"
    assert "#" not in fm


def test_parse_frontmatter_missing(tmp_path):
    """Test parsing file without frontmatter."""
    md = tmp_path / "test.md"
    md.write_text("# Just a regular markdown file\nNo frontmatter here.")
    fm = _parse_frontmatter(md)
    assert fm == {}


def test_parse_frontmatter_malformed_no_closing(tmp_path):
    """Test parsing frontmatter without closing delimiter."""
    md = tmp_path / "test.md"
    md.write_text("""---
segment: 5
title: "Test"
# Missing closing ---
""")
    fm = _parse_frontmatter(md)
    assert fm == {}


def test_parse_frontmatter_empty_lines(tmp_path):
    """Test parsing frontmatter with empty lines."""
    md = tmp_path / "test.md"
    md.write_text("""---
segment: 5

title: "Test"

depends_on: [1, 2]
---
""")
    fm = _parse_frontmatter(md)
    assert fm["segment"] == 5
    assert fm["title"] == "Test"
    assert fm["depends_on"] == [1, 2]


def test_parse_frontmatter_no_colon_lines(tmp_path):
    """Test parsing frontmatter with lines without colons (should be ignored)."""
    md = tmp_path / "test.md"
    md.write_text("""---
segment: 5
invalid line without colon
title: "Test"
---
""")
    fm = _parse_frontmatter(md)
    assert fm["segment"] == 5
    assert fm["title"] == "Test"
    assert len(fm) == 2


def test_parse_frontmatter_unquoted_string(tmp_path):
    """Test parsing frontmatter with unquoted string values."""
    md = tmp_path / "test.md"
    md.write_text("""---
segment: 5
title: Unquoted Title
complexity: Medium
---
""")
    fm = _parse_frontmatter(md)
    assert fm["title"] == "Unquoted Title"
    assert fm["complexity"] == "Medium"


# =============================================================================
# _compute_transitive_dependents tests
# =============================================================================

def test_compute_transitive_dependents_linear_chain():
    """Test computing dependents for linear chain: S1 → S2 → S3."""
    segments = [
        Segment(1, "s1", "S1"),
        Segment(2, "s2", "S2", depends_on=[1]),
        Segment(3, "s3", "S3", depends_on=[2]),
    ]
    _compute_transitive_dependents(segments)

    assert segments[0].dependents == [2]  # S1 is depended on by S2
    assert segments[1].dependents == [3]  # S2 is depended on by S3
    assert segments[2].dependents == []   # S3 has no dependents


def test_compute_transitive_dependents_diamond():
    """Test computing dependents for diamond: S1 → S2, S1 → S3, S2 → S4, S3 → S4."""
    segments = [
        Segment(1, "s1", "S1"),
        Segment(2, "s2", "S2", depends_on=[1]),
        Segment(3, "s3", "S3", depends_on=[1]),
        Segment(4, "s4", "S4", depends_on=[2, 3]),
    ]
    _compute_transitive_dependents(segments)

    assert sorted(segments[0].dependents) == [2, 3]  # S1 is depended on by S2 and S3
    assert segments[1].dependents == [4]              # S2 is depended on by S4
    assert segments[2].dependents == [4]              # S3 is depended on by S4
    assert segments[3].dependents == []               # S4 has no dependents


def test_compute_transitive_dependents_no_dependencies():
    """Test computing dependents when no dependencies exist."""
    segments = [
        Segment(1, "s1", "S1"),
        Segment(2, "s2", "S2"),
        Segment(3, "s3", "S3"),
    ]
    _compute_transitive_dependents(segments)

    for seg in segments:
        assert seg.dependents == []


def test_compute_transitive_dependents_missing_dependency():
    """Test computing dependents when a dependency doesn't exist in the list."""
    segments = [
        Segment(1, "s1", "S1"),
        Segment(3, "s3", "S3", depends_on=[1, 99]),  # 99 doesn't exist
    ]
    _compute_transitive_dependents(segments)

    assert segments[0].dependents == [3]
    assert segments[1].dependents == []


def test_compute_transitive_dependents_clears_existing():
    """Test that computing dependents clears existing values."""
    segments = [
        Segment(1, "s1", "S1", dependents=[99]),  # Pre-existing dependents
        Segment(2, "s2", "S2", depends_on=[1]),
    ]
    _compute_transitive_dependents(segments)

    assert segments[0].dependents == [2]  # Old value cleared
    assert segments[1].dependents == []


# =============================================================================
# _assign_waves tests (topological sort)
# =============================================================================

def test_assign_waves_linear_chain():
    """Test wave assignment for linear chain: S1 → S2 → S3."""
    segments = [
        Segment(1, "s1", "S1"),
        Segment(2, "s2", "S2", depends_on=[1]),
        Segment(3, "s3", "S3", depends_on=[2]),
    ]
    _assign_waves(segments)

    assert segments[0].wave == 1
    assert segments[1].wave == 2
    assert segments[2].wave == 3


def test_assign_waves_parallel():
    """Test wave assignment for parallel segments."""
    segments = [
        Segment(1, "s1", "S1"),
        Segment(2, "s2", "S2"),
        Segment(3, "s3", "S3", depends_on=[1, 2]),
    ]
    _assign_waves(segments)

    assert segments[0].wave == 1
    assert segments[1].wave == 1
    assert segments[2].wave == 2


def test_assign_waves_diamond():
    """Test wave assignment for diamond dependency."""
    segments = [
        Segment(1, "s1", "S1"),
        Segment(2, "s2", "S2", depends_on=[1]),
        Segment(3, "s3", "S3", depends_on=[1]),
        Segment(4, "s4", "S4", depends_on=[2, 3]),
    ]
    _assign_waves(segments)

    assert segments[0].wave == 1
    assert segments[1].wave == 2
    assert segments[2].wave == 2
    assert segments[3].wave == 3


def test_assign_waves_circular_dependency():
    """Test that circular dependencies are detected and raise ValueError."""
    segments = [
        Segment(1, "s1", "S1", depends_on=[2]),
        Segment(2, "s2", "S2", depends_on=[1]),
    ]

    with pytest.raises(ValueError, match="Circular dependency detected"):
        _assign_waves(segments)


def test_assign_waves_self_circular():
    """Test that self-referencing circular dependency is detected."""
    segments = [
        Segment(1, "s1", "S1", depends_on=[1]),
    ]

    with pytest.raises(ValueError, match="Circular dependency detected"):
        _assign_waves(segments)


def test_assign_waves_filters_missing_dependencies():
    """Test that dependencies on non-existent segments are filtered out."""
    segments = [
        Segment(1, "s1", "S1"),
        Segment(3, "s3", "S3", depends_on=[1, 99]),  # 99 doesn't exist
    ]
    _assign_waves(segments)

    assert segments[0].wave == 1
    assert segments[1].wave == 2
    assert segments[1].depends_on == [1]  # 99 was filtered out


def test_assign_waves_no_dependencies():
    """Test wave assignment when no dependencies exist."""
    segments = [
        Segment(1, "s1", "S1"),
        Segment(2, "s2", "S2"),
        Segment(3, "s3", "S3"),
    ]
    _assign_waves(segments)

    for seg in segments:
        assert seg.wave == 1


def test_assign_waves_complex_dag():
    """Test wave assignment for complex DAG."""
    segments = [
        Segment(1, "s1", "S1"),
        Segment(2, "s2", "S2"),
        Segment(3, "s3", "S3", depends_on=[1]),
        Segment(4, "s4", "S4", depends_on=[1, 2]),
        Segment(5, "s5", "S5", depends_on=[3, 4]),
    ]
    _assign_waves(segments)

    assert segments[0].wave == 1  # S1
    assert segments[1].wave == 1  # S2
    assert segments[2].wave == 2  # S3 (depends on S1)
    assert segments[3].wave == 2  # S4 (depends on S1, S2)
    assert segments[4].wave == 3  # S5 (depends on S3, S4)


# =============================================================================
# load_plan tests
# =============================================================================

def test_load_plan_valid(tmp_path):
    """Test loading a valid plan directory."""
    # Create manifest.md
    manifest = tmp_path / "manifest.md"
    manifest.write_text("""---
plan: "Test Plan"
goal: "Test the planner"
---
""")

    # Create segments directory
    segments_dir = tmp_path / "segments"
    segments_dir.mkdir()

    # Create segment files
    seg1 = segments_dir / "01-first.md"
    seg1.write_text("""---
segment: 1
title: "First Segment"
depends_on: []
cycle_budget: 10
---
""")

    seg2 = segments_dir / "02-second.md"
    seg2.write_text("""---
segment: 2
title: "Second Segment"
depends_on: [1]
cycle_budget: 15
risk: 3
---
""")

    meta, segments = load_plan(tmp_path)

    assert meta.title == "Test Plan"
    assert meta.goal == "Test the planner"
    assert len(segments) == 2
    assert segments[0].num == 1
    assert segments[0].title == "First Segment"
    assert segments[0].wave == 1
    assert segments[1].num == 2
    assert segments[1].title == "Second Segment"
    assert segments[1].wave == 2
    assert segments[1].depends_on == [1]


def test_load_plan_missing_manifest(tmp_path):
    """Test loading plan directory without manifest.md."""
    segments_dir = tmp_path / "segments"
    segments_dir.mkdir()

    with pytest.raises(FileNotFoundError, match="No manifest.md"):
        load_plan(tmp_path)


def test_load_plan_missing_segments_dir(tmp_path):
    """Test loading plan directory without segments/ directory."""
    manifest = tmp_path / "manifest.md"
    manifest.write_text("""---
plan: "Test Plan"
---
""")

    with pytest.raises(FileNotFoundError, match="No segments/ directory"):
        load_plan(tmp_path)


def test_load_plan_no_segments(tmp_path):
    """Test loading plan with empty segments directory."""
    manifest = tmp_path / "manifest.md"
    manifest.write_text("""---
plan: "Test Plan"
---
""")

    segments_dir = tmp_path / "segments"
    segments_dir.mkdir()

    with pytest.raises(ValueError, match="No segments found"):
        load_plan(tmp_path)


def test_load_plan_skips_missing_segment_field(tmp_path):
    """Test that files without 'segment' field are skipped."""
    manifest = tmp_path / "manifest.md"
    manifest.write_text("""---
plan: "Test Plan"
---
""")

    segments_dir = tmp_path / "segments"
    segments_dir.mkdir()

    # File without segment field (should be skipped)
    invalid = segments_dir / "00-invalid.md"
    invalid.write_text("""---
title: "No segment field"
---
""")

    # Valid segment
    valid = segments_dir / "01-valid.md"
    valid.write_text("""---
segment: 1
title: "Valid Segment"
---
""")

    meta, segments = load_plan(tmp_path)

    assert len(segments) == 1
    assert segments[0].num == 1


def test_load_plan_non_sequential_numbering(tmp_path):
    """Test that non-sequential segment numbering works."""
    manifest = tmp_path / "manifest.md"
    manifest.write_text("""---
plan: "Test Plan"
---
""")

    segments_dir = tmp_path / "segments"
    segments_dir.mkdir()

    seg1 = segments_dir / "01-first.md"
    seg1.write_text("""---
segment: 1
title: "First"
---
""")

    seg5 = segments_dir / "05-fifth.md"
    seg5.write_text("""---
segment: 5
title: "Fifth"
---
""")

    seg10 = segments_dir / "10-tenth.md"
    seg10.write_text("""---
segment: 10
title: "Tenth"
depends_on: [1, 5]
---
""")

    meta, segments = load_plan(tmp_path)

    assert len(segments) == 3
    assert segments[0].num == 1
    assert segments[0].wave == 1
    assert segments[1].num == 5
    assert segments[1].wave == 1
    assert segments[2].num == 10
    assert segments[2].wave == 2


def test_load_plan_default_values(tmp_path):
    """Test that default values are applied correctly."""
    manifest = tmp_path / "manifest.md"
    manifest.write_text("""---
plan: "Test Plan"
---
""")

    segments_dir = tmp_path / "segments"
    segments_dir.mkdir()

    seg = segments_dir / "01-test.md"
    seg.write_text("""---
segment: 1
---
""")

    meta, segments = load_plan(tmp_path)

    assert segments[0].title == "01-test"  # Uses slug as default
    assert segments[0].cycle_budget == 15
    assert segments[0].risk == 5
    assert segments[0].complexity == "Medium"
    assert segments[0].depends_on == []
    assert segments[0].timeout == 0


def test_load_plan_computes_dependents(tmp_path):
    """Test that load_plan computes transitive dependents."""
    manifest = tmp_path / "manifest.md"
    manifest.write_text("""---
plan: "Test Plan"
---
""")

    segments_dir = tmp_path / "segments"
    segments_dir.mkdir()

    seg1 = segments_dir / "01-first.md"
    seg1.write_text("""---
segment: 1
title: "First"
---
""")

    seg2 = segments_dir / "02-second.md"
    seg2.write_text("""---
segment: 2
title: "Second"
depends_on: [1]
---
""")

    meta, segments = load_plan(tmp_path)

    assert segments[0].dependents == [2]
    assert segments[1].dependents == []
