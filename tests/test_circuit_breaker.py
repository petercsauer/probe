"""Tests for circuit breaker permanent failure detection."""
import sys
from pathlib import Path

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from scripts.orchestrate_v2.runner import CircuitBreaker


def test_circuit_breaker_permanent_patterns():
    """Test that known permanent patterns are detected."""
    cb = CircuitBreaker()

    # Nested session error
    should_retry, reason = cb.should_retry(
        "Error: Claude Code cannot be launched inside another Claude Code session."
    )
    assert should_retry == False
    assert "nested_session" in reason

    # Missing file error
    should_retry, reason = cb.should_retry(
        "FileNotFoundError: [Errno 2] No such file or directory: '/foo/bar.txt'"
    )
    assert should_retry == False
    assert "missing_file" in reason

    # Permission denied
    should_retry, reason = cb.should_retry(
        "PermissionError: [Errno 13] Permission denied: '/etc/shadow'"
    )
    assert should_retry == False
    assert "permission_denied" in reason

    # Syntax error
    should_retry, reason = cb.should_retry(
        "  File 'test.py', line 5\n    print('missing paren'\nSyntaxError: unexpected EOF"
    )
    assert should_retry == False
    assert "syntax_error" in reason


def test_circuit_breaker_transient_allowed():
    """Test that transient errors allow retry."""
    cb = CircuitBreaker()

    # Network timeout
    should_retry, reason = cb.should_retry(
        "TimeoutError: Connection timed out after 30 seconds"
    )
    assert should_retry == True
    assert reason == ""

    # API rate limit
    should_retry, reason = cb.should_retry(
        "HTTP 429: Too Many Requests. Retry after 60 seconds."
    )
    assert should_retry == True

    # Flaky test
    should_retry, reason = cb.should_retry(
        "AssertionError: Expected 5 but got 4 (intermittent timing issue)"
    )
    assert should_retry == True


def test_circuit_breaker_reason():
    """Test that reason clearly identifies pattern."""
    cb = CircuitBreaker()

    should_retry, reason = cb.should_retry("Permission denied")
    assert "permission_denied" in reason
    assert "Permanent failure pattern detected" in reason


def test_circuit_breaker_case_insensitive():
    """Test pattern matching is case insensitive."""
    cb = CircuitBreaker()

    # Lowercase
    should_retry, _ = cb.should_retry("permission denied")
    assert should_retry == False

    # Uppercase
    should_retry, _ = cb.should_retry("PERMISSION DENIED")
    assert should_retry == False

    # Mixed case
    should_retry, _ = cb.should_retry("Permission Denied")
    assert should_retry == False


def test_circuit_breaker_custom_pattern():
    """Test adding custom patterns."""
    cb = CircuitBreaker()

    # Before adding pattern
    should_retry, _ = cb.should_retry("CustomError: Something specific broke")
    assert should_retry == True

    # Add custom pattern
    cb.add_pattern("custom_error", r"CustomError:")

    # After adding pattern
    should_retry, reason = cb.should_retry("CustomError: Something specific broke")
    assert should_retry == False
    assert "custom_error" in reason


if __name__ == "__main__":
    print("Running circuit breaker tests...\n")

    test_circuit_breaker_permanent_patterns()
    print("[OK] test_circuit_breaker_permanent_patterns: PASS")

    test_circuit_breaker_transient_allowed()
    print("[OK] test_circuit_breaker_transient_allowed: PASS")

    test_circuit_breaker_reason()
    print("[OK] test_circuit_breaker_reason: PASS")

    test_circuit_breaker_case_insensitive()
    print("[OK] test_circuit_breaker_case_insensitive: PASS")

    test_circuit_breaker_custom_pattern()
    print("[OK] test_circuit_breaker_custom_pattern: PASS")

    print("\nAll 5 tests passed!")
