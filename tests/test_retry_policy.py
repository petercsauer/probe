"""Tests for retry policy with exponential backoff."""
from scripts.orchestrate_v2.config import RetryPolicy


def test_retry_delay_calculation():
    """Test exponential backoff calculation."""
    policy = RetryPolicy(initial_delay=10, exponential_base=2.0, jitter=False)

    assert policy.get_delay(0) == 10   # 10 * 2^0
    assert policy.get_delay(1) == 20   # 10 * 2^1
    assert policy.get_delay(2) == 40   # 10 * 2^2
    assert policy.get_delay(3) == 80   # 10 * 2^3


def test_exponential_growth_with_max():
    """Test that delay is capped at max_delay."""
    policy = RetryPolicy(initial_delay=100, max_delay=300, exponential_base=2.0, jitter=False)

    assert policy.get_delay(0) == 100  # 100 * 2^0
    assert policy.get_delay(1) == 200  # 100 * 2^1
    assert policy.get_delay(2) == 300  # 100 * 2^2 = 400, but capped at 300
    assert policy.get_delay(3) == 300  # Still capped


def test_jitter_range():
    """Test that jitter adds randomness within expected range."""
    policy = RetryPolicy(initial_delay=100, jitter=True)

    delays = [policy.get_delay(0) for _ in range(100)]

    # All delays should be between 50 and 150 (100 * [0.5, 1.5])
    assert all(50 <= d <= 150 for d in delays)

    # Should have variance (not all the same)
    assert len(set(delays)) > 10


def test_retry_filtering():
    """Test retry_on and no_retry_on filters."""
    policy = RetryPolicy()

    # Should retry
    assert policy.should_retry("timeout") == True
    assert policy.should_retry("failed") == True
    assert policy.should_retry("unknown") == True

    # Should not retry
    assert policy.should_retry("blocked") == False
    assert policy.should_retry("pass") == False


def test_custom_retry_set():
    """Test custom retry configuration."""
    policy = RetryPolicy(
        retry_on={"custom_error"},
        no_retry_on={"permanent_error"}
    )

    assert policy.should_retry("custom_error") == True
    assert policy.should_retry("permanent_error") == False
    assert policy.should_retry("unknown_status") == False  # Not in retry_on
