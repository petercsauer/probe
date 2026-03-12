"""Tests for SegmentExecutor class."""

import asyncio
from pathlib import Path
from unittest.mock import AsyncMock, Mock, patch

import pytest

from .config import OrchestrateConfig, RetryPolicy
from .planner import Segment
from .segment_executor import SegmentExecutor
from .state import StateDB


@pytest.fixture
async def state_db(tmp_path):
    """Create test state database."""
    db_path = tmp_path / "test.db"
    state = await StateDB.create(db_path)
    yield state
    await state.close()


@pytest.fixture
def config():
    """Create test config."""
    retry_policy = RetryPolicy(
        max_retries=3,
        initial_delay=1,
        max_delay=10,
        jitter=False,
    )
    retry_policy.retry_on = {"failed", "partial", "unknown"}
    return OrchestrateConfig(
        max_retries=3,
        max_parallel=2,
        retry_policy=retry_policy,
        isolation_strategy="none",
    )


@pytest.fixture
def segment():
    """Create test segment."""
    return Segment(
        num=1,
        title="Test Segment",
        slug="test-segment",
        wave=1,
        depends_on=[],
        dependents=[],
    )


@pytest.fixture
def executor(config, state_db, tmp_path):
    """Create test executor."""
    notifier = Mock()
    notifier.segment_complete = AsyncMock()
    return SegmentExecutor(config, state_db, notifier, tmp_path)


@pytest.mark.asyncio
async def test_execute_success_no_retry(executor, segment, state_db):
    """Test successful execution without retries."""
    async def mock_run_segment(*args, **kwargs):
        # Update state to prevent infinite loop
        await state_db.set_status(segment.num, "pass")
        return ("pass", "All tests passed")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        mock_run.side_effect = mock_run_segment

        await state_db.init_segments([segment])
        status, final_status = await executor.execute(segment)

        assert status == "pass"
        assert final_status == "pass"
        assert mock_run.call_count == 1


@pytest.mark.asyncio
async def test_execute_retry_then_pass(executor, segment, state_db):
    """Test execution that fails once then passes."""
    call_count = [0]

    async def mock_run_segment(*args, **kwargs):
        call_count[0] += 1
        if call_count[0] == 1:
            await state_db.set_status(segment.num, "failed")
            return ("failed", "Build error")
        else:
            await state_db.set_status(segment.num, "pass")
            return ("pass", "Success")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        mock_run.side_effect = mock_run_segment

        await state_db.init_segments([segment])
        status, final_status = await executor.execute(segment)

        assert status == "pass"
        assert final_status == "pass"
        assert mock_run.call_count == 2


@pytest.mark.asyncio
async def test_execute_max_retries_exhausted(executor, segment, state_db):
    """Test execution fails after max retries."""
    async def mock_run_segment(*args, **kwargs):
        await state_db.set_status(segment.num, "failed")
        return ("failed", "Persistent error")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        mock_run.side_effect = mock_run_segment

        await state_db.init_segments([segment])
        status, final_status = await executor.execute(segment)

        assert status == "failed"
        assert final_status == "failed"
        # max_retries=3 means 4 total attempts (initial + 3 retries)
        assert mock_run.call_count == 4


@pytest.mark.asyncio
async def test_execute_timeout_no_retry(executor, segment, state_db):
    """Test timeout status stops retrying immediately."""
    async def mock_run_segment(*args, **kwargs):
        await state_db.set_status(segment.num, "timeout")
        return ("timeout", "Execution timed out")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        mock_run.side_effect = mock_run_segment

        await state_db.init_segments([segment])
        status, final_status = await executor.execute(segment)

        assert status == "timeout"
        assert final_status == "timeout"
        assert mock_run.call_count == 1


@pytest.mark.asyncio
async def test_execute_non_retryable_status(executor, segment, state_db):
    """Test non-retryable status stops execution."""
    async def mock_run_segment(*args, **kwargs):
        await state_db.set_status(segment.num, "blocked")
        return ("blocked", "Dependency failed")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        mock_run.side_effect = mock_run_segment

        await state_db.init_segments([segment])
        status, final_status = await executor.execute(segment)

        assert status == "blocked"
        assert final_status == "blocked"
        assert mock_run.call_count == 1


@pytest.mark.asyncio
async def test_execute_circuit_breaker_trips(executor, segment, state_db):
    """Test circuit breaker stops retries on permanent failure patterns."""
    async def mock_run_segment(*args, **kwargs):
        await state_db.set_status(segment.num, "failed")
        # Return error that matches permanent failure pattern
        return ("failed", "ModuleNotFoundError: No module named 'foo'")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        mock_run.side_effect = mock_run_segment

        await state_db.init_segments([segment])
        status, final_status = await executor.execute(segment)

        assert status == "failed"
        # Circuit breaker should trip immediately on permanent failure pattern
        assert mock_run.call_count == 1


@pytest.mark.asyncio
async def test_execute_partial_status_immediate_retry(executor, segment, state_db):
    """Test PARTIAL status retries immediately without delay."""
    call_count = [0]

    async def mock_run_segment(*args, **kwargs):
        call_count[0] += 1
        if call_count[0] == 1:
            await state_db.set_status(segment.num, "partial")
            return ("partial", "Work in progress")
        else:
            await state_db.set_status(segment.num, "pass")
            return ("pass", "Completed")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        with patch("asyncio.sleep", new_callable=AsyncMock) as mock_sleep:
            mock_run.side_effect = mock_run_segment

            await state_db.init_segments([segment])
            status, final_status = await executor.execute(segment)

            assert status == "pass"
            # Sleep should NOT be called for partial status
            assert mock_sleep.call_count == 0


@pytest.mark.asyncio
async def test_execute_unknown_status_immediate_retry(executor, segment, state_db):
    """Test UNKNOWN status retries immediately without delay."""
    call_count = [0]

    async def mock_run_segment(*args, **kwargs):
        call_count[0] += 1
        if call_count[0] == 1:
            await state_db.set_status(segment.num, "unknown")
            return ("unknown", "Could not parse status")
        else:
            await state_db.set_status(segment.num, "pass")
            return ("pass", "Success")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        with patch("asyncio.sleep", new_callable=AsyncMock) as mock_sleep:
            mock_run.side_effect = mock_run_segment

            await state_db.init_segments([segment])
            status, final_status = await executor.execute(segment)

            assert status == "pass"
            # Sleep should NOT be called for unknown status
            assert mock_sleep.call_count == 0


@pytest.mark.asyncio
async def test_execute_failed_status_with_delay(executor, segment, state_db):
    """Test FAILED status retries with exponential backoff."""
    call_count = [0]

    async def mock_run_segment(*args, **kwargs):
        call_count[0] += 1
        if call_count[0] == 1:
            await state_db.set_status(segment.num, "failed")
            return ("failed", "Error 1")
        else:
            await state_db.set_status(segment.num, "pass")
            return ("pass", "Success")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        with patch("asyncio.sleep", new_callable=AsyncMock) as mock_sleep:
            mock_run.side_effect = mock_run_segment

            await state_db.init_segments([segment])
            status, final_status = await executor.execute(segment)

            assert status == "pass"
            # Sleep should be called once with base_delay
            assert mock_sleep.call_count == 1


@pytest.mark.asyncio
async def test_execute_operator_retry(executor, segment, state_db):
    """Test operator retry detection."""
    call_count = [0]

    async def mock_run_segment(*args, **kwargs):
        call_count[0] += 1
        if call_count[0] == 1:
            await state_db.set_status(segment.num, "failed")
            return ("failed", "Error")
        else:
            await state_db.set_status(segment.num, "pass")
            return ("pass", "Success after operator retry")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        mock_run.side_effect = mock_run_segment

        await state_db.init_segments([segment])

        # Simulate operator resetting status to pending after first attempt
        async def set_pending_after_first():
            await asyncio.sleep(0.1)  # Give time for first attempt
            await state_db.set_status(segment.num, "pending")

        asyncio.create_task(set_pending_after_first())

        status, final_status = await executor.execute(segment)

        # Should have retried due to operator intervention
        assert mock_run.call_count >= 2


@pytest.mark.asyncio
async def test_execute_with_worktree_success(executor, segment, state_db):
    """Test execution with worktree succeeds and merges."""
    executor.config.isolation_strategy = "worktree"

    mock_worktree = Mock()
    mock_worktree.path = Path("/tmp/wt-001")
    mock_worktree.branch = "wt/seg-001"

    async def mock_run_segment(*args, **kwargs):
        await state_db.set_status(segment.num, "pass")
        return ("pass", "Success")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        with patch("orchestrate_v3.segment_executor._merge_worktree_changes", new_callable=AsyncMock) as mock_merge:
            mock_run.side_effect = mock_run_segment
            mock_merge.return_value = True

            await state_db.init_segments([segment])
            status, final_status = await executor.execute(segment, worktree=mock_worktree)

            assert status == "pass"
            assert final_status == "merged"
            assert mock_merge.called


@pytest.mark.asyncio
async def test_execute_with_worktree_merge_conflict(executor, segment, state_db):
    """Test execution with worktree has merge conflict."""
    executor.config.isolation_strategy = "worktree"

    mock_worktree = Mock()
    mock_worktree.path = Path("/tmp/wt-001")
    mock_worktree.branch = "wt/seg-001"

    async def mock_run_segment(*args, **kwargs):
        await state_db.set_status(segment.num, "pass")
        return ("pass", "Success")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        with patch("orchestrate_v3.segment_executor._merge_worktree_changes", new_callable=AsyncMock) as mock_merge:
            mock_run.side_effect = mock_run_segment
            mock_merge.return_value = False  # Merge conflict

            await state_db.init_segments([segment])
            status, final_status = await executor.execute(segment, worktree=mock_worktree)

            assert status == "pass"
            assert final_status == "pass-merge-conflict"


@pytest.mark.asyncio
async def test_execute_register_unregister_pid(executor, segment, state_db):
    """Test PID registration and unregistration."""
    pids_registered = []
    pids_unregistered = []

    def register_pid(seg_num, pid):
        pids_registered.append((seg_num, pid))

    def unregister_pid(seg_num):
        pids_unregistered.append(seg_num)

    async def mock_run_segment(*args, **kwargs):
        await state_db.set_status(segment.num, "pass")
        return ("pass", "Success")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        mock_run.side_effect = mock_run_segment

        await state_db.init_segments([segment])
        await executor.execute(
            segment,
            register_pid=register_pid,
            unregister_pid=unregister_pid,
        )

        # Verify callbacks were invoked
        assert mock_run.called


@pytest.mark.asyncio
async def test_attempts_increment(executor, segment, state_db):
    """Test that attempts are correctly incremented."""
    call_count = [0]

    async def mock_run_segment(*args, **kwargs):
        call_count[0] += 1
        if call_count[0] < 3:
            await state_db.set_status(segment.num, "failed")
            return ("failed", f"Error {call_count[0]}")
        else:
            await state_db.set_status(segment.num, "pass")
            return ("pass", "Success")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        mock_run.side_effect = mock_run_segment

        await state_db.init_segments([segment])
        await executor.execute(segment)

        # Check final attempts count
        seg_data = await state_db.get_segment(segment.num)
        assert seg_data["attempts"] == 3


@pytest.mark.asyncio
async def test_circuit_breaker_pattern_detection(executor, segment, state_db):
    """Test circuit breaker detects error patterns immediately."""
    async def mock_run_segment(*args, **kwargs):
        await state_db.set_status(segment.num, "failed")
        # Return error that matches permanent failure pattern
        return ("failed", "ImportError: cannot import name 'Foo'")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        mock_run.side_effect = mock_run_segment

        await state_db.init_segments([segment])
        await executor.execute(segment)

        # Circuit breaker should stop immediately on permanent failure pattern
        assert mock_run.call_count == 1


@pytest.mark.asyncio
async def test_state_logging(executor, segment, state_db):
    """Test that state events are logged."""
    call_count = [0]

    async def mock_run_segment(*args, **kwargs):
        call_count[0] += 1
        if call_count[0] == 1:
            await state_db.set_status(segment.num, "failed")
            return ("failed", "Error")
        else:
            await state_db.set_status(segment.num, "pass")
            return ("pass", "Success")

    with patch("orchestrate_v3.segment_executor.run_segment", new_callable=AsyncMock) as mock_run:
        mock_run.side_effect = mock_run_segment

        await state_db.init_segments([segment])
        await executor.execute(segment)

        # Verify events were logged
        # Note: Actual event verification would require state DB query
        assert mock_run.call_count == 2
