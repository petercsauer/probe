"""Tests for status extraction robustness."""
import pytest
from scripts.orchestrate_v2.runner import _extract_status


def test_extract_status_standard():
    """Test standard PASS/PARTIAL/BLOCKED formats."""
    assert _extract_status("**Status:** PASS") == "pass"
    assert _extract_status("Status: PASS") == "pass"
    assert _extract_status("**Status:** PARTIAL") == "partial"
    assert _extract_status("**Status:** BLOCKED") == "blocked"


def test_extract_status_variations():
    """Test common variations are accepted."""
    assert _extract_status("**Status:** COMPLETE") == "pass"
    assert _extract_status("**Status:** SUCCESS") == "pass"
    assert _extract_status("**Status:** DONE") == "pass"
    assert _extract_status("Status: COMPLETE") == "pass"


def test_extract_status_case_insensitive():
    """Test case variations."""
    assert _extract_status("**Status:** complete") == "pass"
    assert _extract_status("**Status:** Complete") == "pass"


def test_extract_status_with_prefix_required():
    """Test that status keyword must have proper prefix."""
    # "PASS" in unrelated context should not match
    assert _extract_status("The test will PASS when ready") == "unknown"
    # But with proper prefix, it should match
    assert _extract_status("Result: **Status:** PASS") == "pass"


def test_extract_status_no_marker():
    """Test unknown when no status marker present."""
    assert _extract_status("Build completed successfully") == "unknown"
    assert _extract_status("") == "unknown"


def test_extract_status_multiple_markers():
    """Test that first match wins."""
    log = "**Status:** BLOCKED\n...\n**Status:** PASS"
    assert _extract_status(log) == "blocked"  # First match


def test_extract_status_with_context():
    """Test extraction from realistic log context."""
    log = """
## Builder Report: Test Segment

**Status:** SUCCESS
**Cycles used:** 5 / 10
**Tests:** 10 passing

### What was built
- Created new module
"""
    assert _extract_status(log) == "pass"


def test_extract_status_partial_variations():
    """Test PARTIAL variations."""
    assert _extract_status("**Status:** IN_PROGRESS") == "partial"
    assert _extract_status("Status: ONGOING") == "partial"
