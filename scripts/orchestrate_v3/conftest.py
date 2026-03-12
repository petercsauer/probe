"""Pytest fixtures for orchestrate_v3 tests."""

import tempfile
from pathlib import Path
from typing import AsyncGenerator
from unittest.mock import AsyncMock, Mock

import pytest

from config import OrchestrateConfig, RetryPolicy
from state import SegmentRow, StateDB


@pytest.fixture
async def temp_dir() -> AsyncGenerator[Path, None]:
    """Create a temporary directory for test isolation.

    Yields:
        Path to temporary directory that will be cleaned up after test.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        yield Path(tmpdir)


@pytest.fixture
async def mock_state_db(temp_dir: Path) -> StateDB:
    """Create an in-memory SQLite StateDB for testing.

    Args:
        temp_dir: Temporary directory for database file

    Returns:
        Initialized StateDB instance with schema created
    """
    db_path = temp_dir / "test_state.db"
    state_db = await StateDB.create(db_path)
    return state_db


@pytest.fixture
def default_config() -> OrchestrateConfig:
    """Create OrchestrateConfig with safe test defaults.

    Returns:
        OrchestrateConfig with reasonable defaults for testing
    """
    return OrchestrateConfig(
        preamble_files=[],
        extra_rules="",
        max_parallel=2,
        segment_timeout=60,
        max_retries=1,
        heartbeat_interval=30,
        isolation_strategy="none",
        isolation_env={},
        gate_command="",
        auth_env={},
        notify_enabled=False,
        ntfy_topic="test-topic",
        notify_verbosity="all",
        notify_max_attempts=1,
        notify_retry_delays=[1],
        monitor_port=0,
        stall_threshold=300,
        network_retry_max=60,
        recovery_enabled=True,
        recovery_max_attempts=1,
        recovery_health_check_timeout=30,
        retry_policy=RetryPolicy(max_retries=1, initial_delay=1, max_delay=10, jitter=False),
        enable_preflight_checks=False,
        preflight_timeout=30,
    )


@pytest.fixture
def mock_segment():
    """Factory fixture for creating test SegmentRow objects.

    Returns:
        Callable that creates SegmentRow with provided or default values

    Example:
        def test_something(mock_segment):
            seg = mock_segment(num=1, slug="s1", title="Test Segment")
            assert seg.num == 1
    """
    def _create_segment(
        num: int = 1,
        slug: str = "test-segment",
        title: str = "Test Segment",
        wave: int = 1,
        status: str = "pending",
        attempts: int = 0,
        started_at: float | None = None,
        finished_at: float | None = None,
        result_json: str | None = None,
        last_seen_at: float | None = None,
        last_activity: str | None = None,
        per_segment_timeout: int | None = None,
    ) -> SegmentRow:
        return SegmentRow(
            num=num,
            slug=slug,
            title=title,
            wave=wave,
            status=status,
            attempts=attempts,
            started_at=started_at,
            finished_at=finished_at,
            result_json=result_json,
            last_seen_at=last_seen_at,
            last_activity=last_activity,
            per_segment_timeout=per_segment_timeout,
        )

    return _create_segment


@pytest.fixture
def mock_notifier():
    """Create a mock notifier that captures notification calls.

    Returns:
        Mock object with send_notification async method that tracks calls

    Example:
        def test_notifications(mock_notifier):
            await mock_notifier.send_notification("test", "message")
            assert mock_notifier.send_notification.call_count == 1
    """
    notifier = Mock()
    notifier.send_notification = AsyncMock()
    notifier.notify_segment_start = AsyncMock()
    notifier.notify_segment_complete = AsyncMock()
    notifier.notify_segment_failed = AsyncMock()
    notifier.notify_wave_complete = AsyncMock()
    notifier.notify_orchestration_complete = AsyncMock()
    return notifier
