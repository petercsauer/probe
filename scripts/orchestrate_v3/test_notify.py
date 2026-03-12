"""Tests for notify.py — ntfy.sh notification outbox."""

from unittest.mock import AsyncMock, Mock, patch

import pytest

from .config import OrchestrateConfig
from .notify import PRIORITY_MAP, Notifier, _send_ntfy
from .state import StateDB


@pytest.mark.asyncio
class TestSendNtfy:
    """Tests for _send_ntfy helper function."""

    async def test_send_success(self):
        """Test successful notification send returns True."""
        mock_response = Mock(status_code=200)
        with patch("httpx.AsyncClient") as mock_client:
            mock_client.return_value.__aenter__.return_value.post = AsyncMock(
                return_value=mock_response
            )
            result = await _send_ntfy(
                topic="test-topic",
                message="Test message",
                title="Test Title",
                priority="high",
                tags="rocket",
                click_url="http://localhost:8080",
            )
            assert result is True
            mock_client.return_value.__aenter__.return_value.post.assert_called_once()
            call_args = mock_client.return_value.__aenter__.return_value.post.call_args
            assert call_args[1]["data"] == b"Test message"
            assert call_args[1]["headers"]["Title"] == "Test Title"
            assert call_args[1]["headers"]["Priority"] == "high"
            assert call_args[1]["headers"]["Tags"] == "rocket"
            assert call_args[1]["headers"]["Click"] == "http://localhost:8080"

    async def test_send_non_200_returns_false(self):
        """Test non-200 status code returns False."""
        mock_response = Mock(status_code=500)
        with patch("httpx.AsyncClient") as mock_client:
            mock_client.return_value.__aenter__.return_value.post = AsyncMock(
                return_value=mock_response
            )
            result = await _send_ntfy(topic="test", message="msg")
            assert result is False

    async def test_send_exception_returns_false(self):
        """Test exception during send returns False."""
        with patch("httpx.AsyncClient") as mock_client:
            mock_client.return_value.__aenter__.return_value.post = AsyncMock(
                side_effect=Exception("Network error")
            )
            result = await _send_ntfy(topic="test", message="msg")
            assert result is False

    async def test_send_minimal_headers(self):
        """Test send with only required parameters."""
        mock_response = Mock(status_code=200)
        with patch("httpx.AsyncClient") as mock_client:
            mock_client.return_value.__aenter__.return_value.post = AsyncMock(
                return_value=mock_response
            )
            result = await _send_ntfy(topic="test", message="msg")
            assert result is True
            call_args = mock_client.return_value.__aenter__.return_value.post.call_args
            headers = call_args[1]["headers"]
            assert "Priority" in headers
            assert "Title" not in headers
            assert "Tags" not in headers
            assert "Click" not in headers


@pytest.mark.asyncio
class TestNotifierInit:
    """Tests for Notifier initialization."""

    async def test_init_enabled_with_topic(self, default_config, mock_state_db):
        """Test notifier enabled when notify_enabled=True and topic provided."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test-topic"
        notifier = Notifier(config, mock_state_db)
        assert notifier._enabled is True
        assert notifier._topic == "test-topic"
        assert notifier._verbosity == "all"
        assert notifier._max_attempts == 1

    async def test_init_disabled_when_notify_disabled(self, default_config, mock_state_db):
        """Test notifier disabled when notify_enabled=False."""
        config = default_config
        config.notify_enabled = False
        config.ntfy_topic = "test-topic"
        notifier = Notifier(config, mock_state_db)
        assert notifier._enabled is False

    async def test_init_disabled_when_no_topic(self, default_config, mock_state_db):
        """Test notifier disabled when ntfy_topic is empty."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = ""
        notifier = Notifier(config, mock_state_db)
        assert notifier._enabled is False

    async def test_init_click_url_with_monitor_port(self, default_config, mock_state_db):
        """Test click_url constructed from monitor_port."""
        config = default_config
        config.monitor_port = 8080
        notifier = Notifier(config, mock_state_db)
        assert notifier._click_url == "http://localhost:8080"

    async def test_init_no_click_url_without_monitor_port(
        self, default_config, mock_state_db
    ):
        """Test click_url empty when monitor_port is 0."""
        config = default_config
        config.monitor_port = 0
        notifier = Notifier(config, mock_state_db)
        assert notifier._click_url == ""


@pytest.mark.asyncio
class TestShouldSend:
    """Tests for _should_send verbosity filtering."""

    async def test_should_send_all_verbosity(self, default_config, mock_state_db):
        """Test 'all' verbosity sends all notification types."""
        config = default_config
        config.notify_verbosity = "all"
        notifier = Notifier(config, mock_state_db)
        assert notifier._should_send("started") is True
        assert notifier._should_send("segment_complete_pass") is True
        assert notifier._should_send("segment_complete_fail") is True
        assert notifier._should_send("wave_complete") is True
        assert notifier._should_send("finished") is True
        assert notifier._should_send("error") is True

    async def test_should_send_failures_only(self, default_config, mock_state_db):
        """Test 'failures_only' verbosity sends only failures."""
        config = default_config
        config.notify_verbosity = "failures_only"
        notifier = Notifier(config, mock_state_db)
        assert notifier._should_send("started") is False
        assert notifier._should_send("segment_complete_pass") is False
        assert notifier._should_send("segment_complete_fail") is True
        assert notifier._should_send("segment_stall") is True
        assert notifier._should_send("gate_fail") is True
        assert notifier._should_send("error") is True
        assert notifier._should_send("finished") is True

    async def test_should_send_waves_only(self, default_config, mock_state_db):
        """Test 'waves_only' verbosity sends only wave-level events."""
        config = default_config
        config.notify_verbosity = "waves_only"
        notifier = Notifier(config, mock_state_db)
        assert notifier._should_send("started") is False
        assert notifier._should_send("segment_complete_pass") is False
        assert notifier._should_send("wave_complete") is True
        assert notifier._should_send("gate_result") is True
        assert notifier._should_send("finished") is True
        assert notifier._should_send("error") is True

    async def test_should_send_final_only(self, default_config, mock_state_db):
        """Test 'final_only' verbosity sends only final events."""
        config = default_config
        config.notify_verbosity = "final_only"
        notifier = Notifier(config, mock_state_db)
        assert notifier._should_send("started") is False
        assert notifier._should_send("segment_complete_pass") is False
        assert notifier._should_send("wave_complete") is False
        assert notifier._should_send("finished") is True
        assert notifier._should_send("error") is True


@pytest.mark.asyncio
class TestNotifierEnqueue:
    """Tests for Notifier.enqueue method."""

    async def test_enqueue_when_disabled(self, default_config, mock_state_db):
        """Test enqueue does nothing when notifier disabled."""
        config = default_config
        config.notify_enabled = False
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.enqueue("test", "message")
        mock_state_db.enqueue_notification.assert_not_called()

    async def test_enqueue_when_filtered_by_verbosity(self, default_config, mock_state_db):
        """Test enqueue does nothing when filtered by verbosity."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        config.notify_verbosity = "final_only"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.enqueue("started", "message")
        mock_state_db.enqueue_notification.assert_not_called()

    async def test_enqueue_calls_state_enqueue(self, default_config, mock_state_db):
        """Test enqueue calls state.enqueue_notification with correct args."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.enqueue("test_kind", "test message", priority="high")
        mock_state_db.enqueue_notification.assert_called_once()
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert call_args[0] == "test_kind"
        assert call_args[1] == "test message"
        assert len(call_args[2]) == 32  # event_key hash length
        assert call_args[3] == "high"

    async def test_enqueue_generates_consistent_event_key(
        self, default_config, mock_state_db
    ):
        """Test event_key is consistent for same kind+message."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.enqueue("kind1", "message1")
        call1_event_key = mock_state_db.enqueue_notification.call_args[0][2]
        mock_state_db.enqueue_notification.reset_mock()
        await notifier.enqueue("kind1", "message1")
        call2_event_key = mock_state_db.enqueue_notification.call_args[0][2]
        assert call1_event_key == call2_event_key


@pytest.mark.asyncio
class TestNotifierMethods:
    """Tests for specific Notifier notification methods."""

    async def test_started_notification(self, default_config, mock_state_db):
        """Test started() enqueues orchestration start notification."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        config.max_parallel = 4
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.started("Test Plan", total=10, waves=3)
        mock_state_db.enqueue_notification.assert_called_once()
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert call_args[0] == "started"
        assert "Test Plan" in call_args[1]
        assert "Segments: 10" in call_args[1]
        assert "Waves: 3" in call_args[1]
        assert "Parallel: 4" in call_args[1]

    async def test_wave_complete_all_passed(self, default_config, mock_state_db):
        """Test wave_complete() with all segments passed."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.wave_complete(2, 5, [(1, "pass"), (2, "pass"), (3, "pass")])
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert call_args[0] == "wave_complete"
        assert "WAVE 2/5 PASS" in call_args[1]
        assert "3/3 passed" in call_args[1]
        assert call_args[3] == "default"  # priority

    async def test_wave_complete_with_failures(self, default_config, mock_state_db):
        """Test wave_complete() with some failures."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.wave_complete(
            1, 3, [(1, "pass"), (2, "failed"), (3, "timeout")]
        )
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert "WAVE 1/3 FAIL" in call_args[1]
        assert "1/3 passed" in call_args[1]
        assert "Failed segments:" in call_args[1]
        assert "S02: FAILED" in call_args[1]
        assert "S03: TIMEOUT" in call_args[1]
        assert call_args[3] == "urgent"  # priority

    async def test_segment_complete_pass(self, default_config, mock_state_db):
        """Test segment_complete() with passing status."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.segment_complete(5, "Test Segment", "pass", "All tests passed")
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert call_args[0] == "segment_complete_pass"
        assert "S05 PASS: Test Segment" in call_args[1]
        assert "All tests passed" in call_args[1]

    async def test_segment_complete_fail(self, default_config, mock_state_db):
        """Test segment_complete() with failing status."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.segment_complete(3, "Failed Segment", "failed", "Error occurred")
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert call_args[0] == "segment_complete_fail"
        assert "S03 FAILED: Failed Segment" in call_args[1]
        assert "Error occurred" in call_args[1]
        assert call_args[3] == "urgent"  # from PRIORITY_MAP

    async def test_gate_result_pass(self, default_config, mock_state_db):
        """Test gate_result() with passing gate."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.gate_result(2, passed=True, detail="All checks passed")
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert call_args[0] == "gate_result"
        assert "GATE Wave 2: PASS" in call_args[1]
        assert call_args[3] == "low"

    async def test_gate_result_fail(self, default_config, mock_state_db):
        """Test gate_result() with failing gate."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.gate_result(3, passed=False, detail="Test failures detected")
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert call_args[0] == "gate_fail"
        assert "GATE Wave 3: FAIL" in call_args[1]
        assert "Test failures detected" in call_args[1]
        assert call_args[3] == "urgent"

    async def test_stall_notification(self, default_config, mock_state_db):
        """Test stall() enqueues stall notification."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.stall(7, minutes=15, activity="Waiting for API response")
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert call_args[0] == "segment_stall"
        assert "STALL: S07 (15min no output)" in call_args[1]
        assert "Waiting for API response" in call_args[1]

    async def test_network_down_notification(self, default_config, mock_state_db):
        """Test network_down() enqueues network outage notification."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.network_down(waited_sec=120)
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert call_args[0] == "network_down"
        assert "NETWORK DOWN: Unreachable for 120s" in call_args[1]

    async def test_finished_all_passed(self, default_config, mock_state_db):
        """Test finished() with all segments passed."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.finished("Test Plan", progress={"pass": 10})
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert call_args[0] == "finished"
        assert "COMPLETE: Test Plan" in call_args[1]
        assert "Status: SUCCESS (10/10 passed)" in call_args[1]

    async def test_finished_partial(self, default_config, mock_state_db):
        """Test finished() with partial completion."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.finished(
            "Test Plan", progress={"pass": 8, "failed": 1, "timeout": 1}
        )
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert "Status: PARTIAL (8/10 passed)" in call_args[1]
        assert "failed: 1" in call_args[1]
        assert "pass: 8" in call_args[1]
        assert "timeout: 1" in call_args[1]

    async def test_error_notification(self, default_config, mock_state_db):
        """Test error() enqueues error notification."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.error("Fatal error: Database connection lost")
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert call_args[0] == "error"
        assert "ERROR: Fatal error: Database connection lost" in call_args[1]
        assert call_args[3] == "urgent"

    async def test_heartbeat_notification(self, default_config, mock_state_db):
        """Test heartbeat() enqueues heartbeat notification."""
        config = default_config
        config.notify_enabled = True
        config.ntfy_topic = "test"
        mock_state_db.enqueue_notification = AsyncMock()
        notifier = Notifier(config, mock_state_db)
        await notifier.heartbeat("Wave 3/5 - S07 running")
        call_args = mock_state_db.enqueue_notification.call_args[0]
        assert call_args[0] == "heartbeat"
        assert "STATUS: Wave 3/5 - S07 running" in call_args[1]
        assert call_args[3] == "min"


def test_priority_map():
    """Test PRIORITY_MAP contains expected status mappings."""
    assert PRIORITY_MAP["pass"] == "default"
    assert PRIORITY_MAP["partial"] == "high"
    assert PRIORITY_MAP["blocked"] == "urgent"
    assert PRIORITY_MAP["failed"] == "urgent"
    assert PRIORITY_MAP["timeout"] == "high"
    assert PRIORITY_MAP["stall"] == "high"
    assert PRIORITY_MAP["error"] == "urgent"
