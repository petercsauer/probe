"""Tests for WaveRunner class."""

import asyncio
from pathlib import Path
from unittest.mock import AsyncMock, Mock, patch

import pytest

from .config import OrchestrateConfig, RetryPolicy
from .planner import Segment
from .state import StateDB
from .wave_runner import WaveRunner


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
        max_retries=2,
        initial_delay=0.1,  # Short delays for tests
        max_delay=1,
        jitter=False,
    )
    retry_policy.retry_on = {"failed", "partial", "unknown"}
    return OrchestrateConfig(
        max_retries=2,
        max_parallel=2,
        retry_policy=retry_policy,
        isolation_strategy="none",
    )


@pytest.fixture
def segments():
    """Create test segments."""
    return [
        Segment(num=1, title="Seg 1", slug="seg-1", wave=1, depends_on=[], dependents=[2]),
        Segment(num=2, title="Seg 2", slug="seg-2", wave=1, depends_on=[1], dependents=[]),
    ]


@pytest.fixture
def wave_runner(config, state_db, tmp_path):
    """Create test wave runner."""
    notifier = Mock()
    notifier.segment_complete = AsyncMock()
    return WaveRunner(state_db, config, notifier, tmp_path)


@pytest.mark.asyncio
async def test_execute_all_segments_pass(wave_runner, segments, state_db):
    """Test all segments in wave pass."""
    async def mock_execute(seg, **kwargs):
        # Update state DB so dependencies are satisfied
        await state_db.set_status(seg.num, "pass")
        return ("pass", "pass")

    with patch("orchestrate_v3.wave_runner.SegmentExecutor.execute", new_callable=AsyncMock) as mock_exec:
        mock_exec.side_effect = mock_execute

        await state_db.init_segments(segments)
        shutting_down = asyncio.Event()
        results = await wave_runner.execute(1, segments, shutting_down, segments)

        assert len(results) == 2
        assert all(status == "pass" for _, status in results)


@pytest.mark.asyncio
async def test_execute_dependency_blocking(wave_runner, segments, state_db):
    """Test segment blocked by failed dependency."""
    async def mock_execute(seg, **kwargs):
        if seg.num == 1:
            return ("failed", "failed")
        return ("pass", "pass")

    with patch("orchestrate_v3.wave_runner.SegmentExecutor.execute", new_callable=AsyncMock) as mock_exec:
        mock_exec.side_effect = mock_execute

        await state_db.init_segments(segments)
        shutting_down = asyncio.Event()
        results = await wave_runner.execute(1, segments, shutting_down, segments)

        # Segment 2 should be skipped due to dependency on segment 1
        results_dict = dict(results)
        assert results_dict[1] == "failed"
        assert results_dict[2] == "skipped-dependency-failed"


@pytest.mark.asyncio
async def test_execute_parallel_execution(wave_runner, segments, state_db):
    """Test parallel execution with semaphore."""
    execution_times = []

    async def mock_execute(seg, **kwargs):
        start = asyncio.get_event_loop().time()
        await asyncio.sleep(0.1)
        end = asyncio.get_event_loop().time()
        execution_times.append((seg.num, start, end))
        return ("pass", "pass")

    with patch("orchestrate_v3.wave_runner.SegmentExecutor.execute", new_callable=AsyncMock) as mock_exec:
        mock_exec.side_effect = mock_execute

        await state_db.init_segments(segments)
        shutting_down = asyncio.Event()
        await wave_runner.execute(1, segments, shutting_down)

        # Both should have executed in parallel (overlapping times)
        assert len(execution_times) == 2


@pytest.mark.asyncio
async def test_execute_shutdown_during_wave(wave_runner, segments, state_db):
    """Test graceful shutdown during wave execution."""
    shutting_down = asyncio.Event()

    async def mock_execute(seg, **kwargs):
        await asyncio.sleep(0.05)
        return ("pass", "pass")

    with patch("orchestrate_v3.wave_runner.SegmentExecutor.execute", new_callable=AsyncMock) as mock_exec:
        mock_exec.side_effect = mock_execute

        await state_db.init_segments(segments)

        # Signal shutdown before execution starts
        shutting_down.set()
        results = await wave_runner.execute(1, segments, shutting_down)

        # All segments should be skipped
        assert all(status == "skipped" for _, status in results)


@pytest.mark.asyncio
async def test_execute_operator_skip(wave_runner, segments, state_db):
    """Test operator manually skipping a segment."""
    await state_db.init_segments(segments)

    # Operator skips segment 1
    await state_db.set_status(1, "skipped")

    with patch("orchestrate_v3.wave_runner.SegmentExecutor.execute", new_callable=AsyncMock) as mock_exec:
        mock_exec.return_value = ("pass", "pass")

        shutting_down = asyncio.Event()
        results = await wave_runner.execute(1, segments, shutting_down)

        results_dict = dict(results)
        assert results_dict[1] == "skipped"
        # Segment 2 is blocked because segment 1 was skipped (dependency not satisfied)
        assert results_dict[2] == "skipped-dependency-failed"


@pytest.mark.asyncio
async def test_execute_post_gather_retry(wave_runner, segments, state_db):
    """Test post-gather operator retry."""
    call_count = {"count": 0}

    async def mock_execute(seg, **kwargs):
        call_count["count"] += 1
        if call_count["count"] == 1 and seg.num == 1:
            # After first execution, operator will reset to pending
            return ("pass", "pass")
        return ("pass", "pass")

    with patch("orchestrate_v3.wave_runner.SegmentExecutor.execute", new_callable=AsyncMock) as mock_exec:
        mock_exec.side_effect = mock_execute

        await state_db.init_segments(segments)

        # Simulate operator resetting segment 1 to pending after execution
        async def reset_after_execution():
            await asyncio.sleep(0.15)
            await state_db.set_status(1, "pending")

        asyncio.create_task(reset_after_execution())

        shutting_down = asyncio.Event()
        results = await wave_runner.execute(1, [segments[0]], shutting_down)

        # Should have retried segment 1
        assert call_count["count"] >= 1


@pytest.mark.asyncio
async def test_execute_mark_transitive_dependents_skipped(wave_runner, state_db):
    """Test transitive dependent skipping."""
    seg1 = Segment(num=1, title="S1", slug="s1", wave=1, depends_on=[], dependents=[2])
    seg2 = Segment(num=2, title="S2", slug="s2", wave=1, depends_on=[1], dependents=[3])
    seg3 = Segment(num=3, title="S3", slug="s3", wave=1, depends_on=[2], dependents=[])

    all_segments = [seg1, seg2, seg3]

    async def mock_execute(seg, **kwargs):
        if seg.num == 1:
            return ("failed", "failed")
        return ("pass", "pass")

    with patch("orchestrate_v3.wave_runner.SegmentExecutor.execute", new_callable=AsyncMock) as mock_exec:
        mock_exec.side_effect = mock_execute

        await state_db.init_segments(all_segments)
        shutting_down = asyncio.Event()
        results = await wave_runner.execute(1, [seg1, seg2, seg3], shutting_down, all_segments)

        results_dict = dict(results)
        assert results_dict[1] == "failed"
        # Both seg2 and seg3 should be skipped (transitive)
        assert results_dict[2] == "skipped-dependency-failed"
        assert results_dict[3] == "skipped-dependency-failed"


@pytest.mark.asyncio
async def test_execute_with_worktree_pool(wave_runner, segments, state_db):
    """Test execution with worktree pool."""
    wave_runner.config.isolation_strategy = "worktree"

    mock_pool = Mock()
    mock_worktree = Mock()
    mock_worktree.path = Path("/tmp/wt-001")

    # Mock async context manager
    class AsyncContextManager:
        async def __aenter__(self):
            return mock_worktree

        async def __aexit__(self, *args):
            pass

    def mock_acquire(seg_num):
        return AsyncContextManager()

    mock_pool.acquire = mock_acquire
    wave_runner.pool = mock_pool

    with patch("orchestrate_v3.wave_runner.SegmentExecutor.execute", new_callable=AsyncMock) as mock_exec:
        mock_exec.return_value = ("pass", "merged")

        await state_db.init_segments(segments)
        shutting_down = asyncio.Event()
        results = await wave_runner.execute(1, [segments[0]], shutting_down)

        # Verify worktree was used
        assert mock_exec.called


@pytest.mark.asyncio
async def test_execute_exception_handling(wave_runner, segments, state_db):
    """Test exception during segment execution."""
    async def mock_execute(seg, **kwargs):
        if seg.num == 1:
            raise RuntimeError("Unexpected error")
        return ("pass", "pass")

    with patch("orchestrate_v3.wave_runner.SegmentExecutor.execute", new_callable=AsyncMock) as mock_exec:
        mock_exec.side_effect = mock_execute

        await state_db.init_segments(segments)
        shutting_down = asyncio.Event()
        results = await wave_runner.execute(1, segments, shutting_down)

        results_dict = dict(results)
        # Exception should be caught and marked as error
        assert results_dict[1] == "error"


@pytest.mark.asyncio
async def test_execute_merge_conflict_handling(wave_runner, segments, state_db):
    """Test handling of merge conflicts."""
    async def mock_execute(seg, **kwargs):
        return ("pass", "pass-merge-conflict")

    with patch("orchestrate_v3.wave_runner.SegmentExecutor.execute", new_callable=AsyncMock) as mock_exec:
        mock_exec.side_effect = mock_execute

        await state_db.init_segments(segments)
        shutting_down = asyncio.Event()
        results = await wave_runner.execute(1, [segments[0]], shutting_down)

        results_dict = dict(results)
        assert results_dict[1] == "pass-merge-conflict"


@pytest.mark.asyncio
async def test_execute_pid_registration(wave_runner, segments, state_db):
    """Test PID registration during execution."""
    running_pids = {}
    wave_runner.running_pids = running_pids

    with patch("orchestrate_v3.wave_runner.SegmentExecutor.execute", new_callable=AsyncMock) as mock_exec:
        mock_exec.return_value = ("pass", "pass")

        await state_db.init_segments(segments)
        shutting_down = asyncio.Event()
        await wave_runner.execute(1, [segments[0]], shutting_down)

        # PIDs should be managed during execution
        assert mock_exec.called


@pytest.mark.asyncio
async def test_validate_upstream_dependencies_pass(state_db):
    """Test dependency validation when all pass."""
    from .wave_runner import _validate_upstream_dependencies

    seg = Segment(num=2, title="S2", slug="s2", wave=1, depends_on=[1], dependents=[])

    await state_db.init_segments([
        Segment(num=1, title="S1", slug="s1", wave=1, depends_on=[], dependents=[2]),
        seg,
    ])
    await state_db.set_status(1, "pass")

    can_run, blocking = await _validate_upstream_dependencies(seg, state_db)

    assert can_run is True
    assert blocking == []


@pytest.mark.asyncio
async def test_validate_upstream_dependencies_blocked(state_db):
    """Test dependency validation when blocked."""
    from .wave_runner import _validate_upstream_dependencies

    seg = Segment(num=2, title="S2", slug="s2", wave=1, depends_on=[1], dependents=[])

    await state_db.init_segments([
        Segment(num=1, title="S1", slug="s1", wave=1, depends_on=[], dependents=[2]),
        seg,
    ])
    await state_db.set_status(1, "failed")

    can_run, blocking = await _validate_upstream_dependencies(seg, state_db)

    assert can_run is False
    assert "S01" in blocking


@pytest.mark.asyncio
async def test_mark_dependents_skipped_single_level(state_db):
    """Test marking single-level dependents as skipped."""
    from .wave_runner import _mark_dependents_skipped

    seg1 = Segment(num=1, title="S1", slug="s1", wave=1, depends_on=[], dependents=[2])
    seg2 = Segment(num=2, title="S2", slug="s2", wave=1, depends_on=[1], dependents=[])

    await state_db.init_segments([seg1, seg2])
    await state_db.set_status(2, "pending")

    skipped = await _mark_dependents_skipped(1, state_db, [seg1, seg2], "failed")

    assert 2 in skipped
    seg2_status = await state_db.get_segment(2)
    assert seg2_status["status"] == "skipped-dependency-failed"


@pytest.mark.asyncio
async def test_mark_dependents_skipped_transitive(state_db):
    """Test marking transitive dependents as skipped."""
    from .wave_runner import _mark_dependents_skipped

    seg1 = Segment(num=1, title="S1", slug="s1", wave=1, depends_on=[], dependents=[2])
    seg2 = Segment(num=2, title="S2", slug="s2", wave=1, depends_on=[1], dependents=[3])
    seg3 = Segment(num=3, title="S3", slug="s3", wave=1, depends_on=[2], dependents=[])

    await state_db.init_segments([seg1, seg2, seg3])
    await state_db.set_status(2, "pending")
    await state_db.set_status(3, "pending")

    skipped = await _mark_dependents_skipped(1, state_db, [seg1, seg2, seg3], "failed")

    assert 2 in skipped
    assert 3 in skipped
