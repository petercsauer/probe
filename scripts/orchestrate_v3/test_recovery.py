"""Tests for recovery agent: workspace health checks and victim detection."""

import asyncio
import json
import tempfile
from pathlib import Path
from unittest.mock import AsyncMock, Mock, patch

import pytest

from recovery import RecoveryAgent, RecoveryConfig
from state import SegmentRow


class TestRecoveryConfig:
    """Test RecoveryConfig initialization."""

    def test_default_config(self):
        """Test RecoveryConfig with default values."""
        config = RecoveryConfig()
        assert config.enabled is True
        assert config.health_check_timeout == 300
        assert config.victim_markers == [
            "pre-existing errors",
            "my code is correct",
            "blocked by S",
        ]

    def test_custom_config(self):
        """Test RecoveryConfig with custom values."""
        config = RecoveryConfig(
            enabled=False,
            health_check_timeout=600,
            victim_markers=["custom marker"],
        )
        assert config.enabled is False
        assert config.health_check_timeout == 600
        assert config.victim_markers == ["custom marker"]


class TestWorkspaceHealth:
    """Test workspace health checking via cargo check."""

    async def test_workspace_health_pass(self):
        """Test that clean cargo check returns healthy=True with empty errors."""
        # Mock state and config
        state = Mock()
        config = Mock()

        agent = RecoveryAgent(state, config)

        # Mock successful cargo check with no errors
        mock_proc = AsyncMock()
        mock_proc.returncode = 0
        # Simulate cargo check JSON output with only success messages
        stdout_data = b'{"reason":"compiler-artifact","target":{"name":"prb-core"}}\n'
        mock_proc.communicate = AsyncMock(return_value=(stdout_data, b""))

        with patch("asyncio.create_subprocess_exec", return_value=mock_proc):
            healthy, errors = await agent.check_workspace_health()

        assert healthy is True, "Expected healthy=True for clean workspace"
        assert errors == [], f"Expected no errors, got {errors}"

    async def test_workspace_health_fail(self):
        """Test that cargo errors are detected and returned."""
        state = Mock()
        config = Mock()

        agent = RecoveryAgent(state, config)

        # Mock cargo check with compilation error
        mock_proc = AsyncMock()
        mock_proc.returncode = 1

        # Simulate cargo check JSON with error message
        error_msg = {
            "reason": "compiler-message",
            "message": {
                "level": "error",
                "rendered": "error[E0425]: cannot find value `undefined_var` in this scope\n --> src/main.rs:5:5\n",
            },
        }
        stdout_data = json.dumps(error_msg).encode() + b"\n"
        mock_proc.communicate = AsyncMock(return_value=(stdout_data, b""))

        with patch("asyncio.create_subprocess_exec", return_value=mock_proc):
            healthy, errors = await agent.check_workspace_health()

        assert healthy is False, "Expected healthy=False for failing workspace"
        assert len(errors) > 0, "Expected at least one error message"
        assert "E0425" in errors[0], f"Expected error code in message: {errors}"

    async def test_workspace_health_timeout(self):
        """Test that timeout is handled gracefully."""
        state = Mock()
        config = Mock()

        agent = RecoveryAgent(state, config)
        agent.recovery_config.health_check_timeout = 0.1  # Very short timeout

        # Mock a slow process that never completes
        mock_proc = Mock()
        async def slow_communicate():
            await asyncio.sleep(10)  # Longer than timeout
            return (b"", b"")
        mock_proc.communicate = slow_communicate
        mock_proc.kill = Mock()  # Synchronous mock for kill
        mock_proc.wait = AsyncMock()  # Async mock for wait

        with patch("asyncio.create_subprocess_exec", return_value=mock_proc):
            healthy, errors = await agent.check_workspace_health()

        assert healthy is False, "Expected healthy=False on timeout"
        assert len(errors) == 1, f"Expected 1 error, got {len(errors)}"
        assert "timeout" in errors[0].lower(), f"Expected timeout message: {errors}"

    async def test_workspace_health_exception(self):
        """Test that subprocess exceptions are handled gracefully."""
        state = Mock()
        config = Mock()

        agent = RecoveryAgent(state, config)

        # Mock create_subprocess_exec to raise exception
        with patch(
            "asyncio.create_subprocess_exec",
            side_effect=FileNotFoundError("cargo not found"),
        ):
            healthy, errors = await agent.check_workspace_health()

        assert healthy is False, "Expected healthy=False on exception"
        assert len(errors) == 1, f"Expected 1 error, got {len(errors)}"
        assert "exception" in errors[0].lower(), f"Expected exception message: {errors}"

    async def test_workspace_health_stderr_fallback(self):
        """Test that stderr is used when no JSON errors but non-zero exit."""
        state = Mock()
        config = Mock()

        agent = RecoveryAgent(state, config)

        # Mock cargo check with non-zero exit but no JSON errors
        mock_proc = AsyncMock()
        mock_proc.returncode = 1
        # Empty JSON output but error in stderr
        mock_proc.communicate = AsyncMock(
            return_value=(b"", b"error: could not compile `prb-core`")
        )

        with patch("asyncio.create_subprocess_exec", return_value=mock_proc):
            healthy, errors = await agent.check_workspace_health()

        assert healthy is False, "Expected healthy=False"
        assert len(errors) > 0, "Expected errors from stderr"
        assert "could not compile" in errors[0], f"Expected stderr content: {errors}"


class TestIdentifyVictims:
    """Test cascade victim detection from builder reports."""

    async def test_identify_victims(self):
        """Test that victim markers are detected in builder logs."""
        state = Mock()
        config = Mock()

        agent = RecoveryAgent(state, config)

        # Create temp directory for test logs
        with tempfile.TemporaryDirectory() as tmpdir:
            log_dir = Path(tmpdir)
            orig_cwd = Path.cwd()

            try:
                # Create test log files
                # S01: BLOCKED with victim marker
                s01_log = log_dir / "S01.log"
                s01_log.write_text(
                    "## Builder Report: Segment 1\n\n"
                    "**Status:** BLOCKED\n\n"
                    "The build fails with pre-existing errors in the workspace.\n"
                    "My code is correct but cannot compile due to S00 failures.\n"
                )

                # S02: PASS - should not be identified
                s02_log = log_dir / "S02.log"
                s02_log.write_text(
                    "## Builder Report: Segment 2\n\n"
                    "**Status:** PASS\n\n"
                    "All tests passing.\n"
                )

                # S03: PARTIAL with victim marker
                s03_log = log_dir / "S03.log"
                s03_log.write_text(
                    "## Builder Report: Segment 3\n\n"
                    "**Status:** PARTIAL\n\n"
                    "I believe my code is correct but blocked by S01.\n"
                )

                # S04: BLOCKED without victim marker (genuine failure)
                s04_log = log_dir / "S04.log"
                s04_log.write_text(
                    "## Builder Report: Segment 4\n\n"
                    "**Status:** BLOCKED\n\n"
                    "Failed to implement the required feature. Logic error in algorithm.\n"
                )

                # Create mock segment rows
                wave_segments = [
                    SegmentRow(
                        num=1, slug="s1", title="Segment 1", wave=1,
                        status="blocked", attempts=1
                    ),
                    SegmentRow(
                        num=2, slug="s2", title="Segment 2", wave=1,
                        status="pass", attempts=1
                    ),
                    SegmentRow(
                        num=3, slug="s3", title="Segment 3", wave=1,
                        status="partial", attempts=1
                    ),
                    SegmentRow(
                        num=4, slug="s4", title="Segment 4", wave=1,
                        status="blocked", attempts=1
                    ),
                ]

                # Create logs subdirectory
                logs_subdir = log_dir / "logs"
                logs_subdir.mkdir()

                # Move logs to logs/ subdirectory
                (logs_subdir / "S01.log").write_text(s01_log.read_text())
                (logs_subdir / "S02.log").write_text(s02_log.read_text())
                (logs_subdir / "S03.log").write_text(s03_log.read_text())
                (logs_subdir / "S04.log").write_text(s04_log.read_text())

                # Change to temp dir so logs/ path works
                import os
                os.chdir(tmpdir)

                victims = await agent.identify_cascade_victims(wave_segments)

                # Should identify S01 and S03 (both have victim markers)
                # S02 is PASS (skipped)
                # S04 is BLOCKED but no victim marker (not a victim)
                assert 1 in victims, f"Expected S01 in victims: {victims}"
                assert 3 in victims, f"Expected S03 in victims: {victims}"
                assert 2 not in victims, f"S02 should not be victim (passed): {victims}"
                assert 4 not in victims, f"S04 should not be victim (no marker): {victims}"
                assert len(victims) == 2, f"Expected exactly 2 victims: {victims}"


            finally:
                import os
                os.chdir(orig_cwd)

    async def test_identify_victims_empty_wave(self):
        """Test that empty wave returns no victims."""
        state = Mock()
        config = Mock()

        agent = RecoveryAgent(state, config)

        victims = await agent.identify_cascade_victims([])

        assert victims == [], f"Expected no victims for empty wave: {victims}"

    async def test_identify_victims_no_logs(self):
        """Test that missing log files are handled gracefully."""
        state = Mock()
        config = Mock()

        agent = RecoveryAgent(state, config)

        with tempfile.TemporaryDirectory() as tmpdir:
            orig_cwd = Path.cwd()
            try:
                import os
                os.chdir(tmpdir)

                # Segment with no corresponding log file
                wave_segments = [
                    SegmentRow(
                        num=99, slug="s99", title="Missing Log", wave=1,
                        status="blocked", attempts=1
                    ),
                ]

                victims = await agent.identify_cascade_victims(wave_segments)

                # Should handle gracefully and return no victims
                assert victims == [], f"Expected no victims when logs missing: {victims}"

            finally:
                os.chdir(orig_cwd)


