"""Tests for SignalHandler."""

from __future__ import annotations

import asyncio
import signal
from unittest.mock import Mock, patch

import pytest

from .signal_handler import SignalHandler


@pytest.fixture
def signal_handler():
    """Create SignalHandler instance."""
    return SignalHandler()


def test_signal_handler_init(signal_handler):
    """Test SignalHandler initialization."""
    assert not signal_handler.is_shutting_down()
    assert not signal_handler.shutting_down.is_set()
    assert not signal_handler.worker_stop.is_set()


def test_signal_handler_shutdown(signal_handler):
    """Test shutdown method."""
    signal_handler.shutdown()

    assert signal_handler.is_shutting_down()
    assert signal_handler.shutting_down.is_set()
    assert signal_handler.worker_stop.is_set()


def test_signal_handler_is_shutting_down_initially_false(signal_handler):
    """Test is_shutting_down returns False initially."""
    assert signal_handler.is_shutting_down() is False


def test_signal_handler_is_shutting_down_after_shutdown(signal_handler):
    """Test is_shutting_down returns True after shutdown."""
    signal_handler.shutdown()
    assert signal_handler.is_shutting_down() is True


@pytest.mark.asyncio
async def test_signal_handler_register_handlers(signal_handler):
    """Test register_handlers registers signal handlers."""
    loop = asyncio.get_running_loop()

    with patch.object(loop, 'add_signal_handler') as mock_add:
        signal_handler.register_handlers(loop)

        # Should register both SIGINT and SIGTERM
        assert mock_add.call_count == 2
        calls = mock_add.call_args_list
        registered_signals = [call[0][0] for call in calls]

        assert signal.SIGINT in registered_signals
        assert signal.SIGTERM in registered_signals


@pytest.mark.asyncio
async def test_signal_handler_events_are_separate(signal_handler):
    """Test shutting_down and worker_stop are separate events."""
    # Initially both are not set
    assert not signal_handler.shutting_down.is_set()
    assert not signal_handler.worker_stop.is_set()

    # After shutdown, both should be set
    signal_handler.shutdown()
    assert signal_handler.shutting_down.is_set()
    assert signal_handler.worker_stop.is_set()
