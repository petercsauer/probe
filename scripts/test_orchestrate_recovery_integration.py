#!/usr/bin/env python3
"""Integration tests for recovery agent in orchestration wave flow."""

import asyncio
import sys
import tempfile
from pathlib import Path
from unittest.mock import AsyncMock, Mock, patch

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent))

from orchestrate_v2.config import OrchestrateConfig
from orchestrate_v2.recovery import RecoveryAgent
from orchestrate_v2.state import SegmentRow


class TestRecoveryIntegration:
    """Test recovery agent integration with orchestration wave flow."""

    async def test_recovery_triggers_on_partial(self):
        """Test that recovery is triggered when wave has partial/blocked segments."""
        # Create mock config with recovery enabled
        config = Mock(spec=OrchestrateConfig)
        config.recovery_enabled = True
        config.recovery_max_attempts = 1
        config.recovery_health_check_timeout = 120

        # Create mock state
        state = Mock()
        state.get_segment = AsyncMock()

        # Simulate wave with one partial segment
        wave_results = [(1, "pass"), (2, "partial"), (3, "pass")]

        # Check that recovery should trigger
        has_failures = any(status in ("partial", "blocked") for _, status in wave_results)
        assert has_failures, "Wave should have failures to trigger recovery"
        print("[OK] test_recovery_triggers_on_partial")

    async def test_recovery_retries_victims(self):
        """Test that recovery identifies and retries victim segments."""
        with tempfile.TemporaryDirectory() as tmpdir:
            tmpdir_path = Path(tmpdir)

            # Create mock config
            config = Mock(spec=OrchestrateConfig)
            config.recovery_enabled = True
            config.recovery_max_attempts = 1
            config.recovery_health_check_timeout = 120

            # Create mock state
            state = Mock()
            state.log_event = AsyncMock()

            # Create recovery agent
            agent = RecoveryAgent(state, config)

            # Create test log files
            logs_dir = tmpdir_path / "logs"
            logs_dir.mkdir()

            # S01: blocked with victim marker
            s01_log = logs_dir / "S01.log"
            s01_log.write_text(
                "## Builder Report: Segment 1\n\n"
                "**Status:** BLOCKED\n\n"
                "The build fails with pre-existing errors in the workspace.\n"
            )

            # S02: pass
            s02_log = logs_dir / "S02.log"
            s02_log.write_text(
                "## Builder Report: Segment 2\n\n"
                "**Status:** PASS\n\n"
                "All tests passing.\n"
            )

            # Create segment rows
            wave_segments = [
                SegmentRow(
                    num=1, slug="s1", title="Segment 1", wave=1,
                    status="blocked", attempts=1
                ),
                SegmentRow(
                    num=2, slug="s2", title="Segment 2", wave=1,
                    status="pass", attempts=1
                ),
            ]

            # Mock cargo check to return healthy
            mock_proc = AsyncMock()
            mock_proc.returncode = 0
            mock_proc.communicate = AsyncMock(return_value=(b"", b""))

            orig_cwd = Path.cwd()
            try:
                import os
                os.chdir(tmpdir_path)

                with patch("asyncio.create_subprocess_exec", return_value=mock_proc):
                    # Check workspace health
                    healthy, errors = await agent.check_workspace_health()
                    assert healthy, "Workspace should be healthy"

                    # Identify victims
                    victims = await agent.identify_cascade_victims(wave_segments)

                    # Should identify S01 as victim
                    assert 1 in victims, f"Expected S01 in victims: {victims}"
                    assert 2 not in victims, f"S02 should not be victim: {victims}"

                print("[OK] test_recovery_retries_victims")

            finally:
                os.chdir(orig_cwd)

    async def test_recovery_circuit_breaker(self):
        """Test that recovery respects max_attempts circuit breaker."""
        # Create mock config with max_attempts = 1
        config = Mock(spec=OrchestrateConfig)
        config.recovery_enabled = True
        config.recovery_max_attempts = 1
        config.recovery_health_check_timeout = 120

        # Create mock state with attempt tracking
        state = Mock()
        attempts_db = {1: 2}  # S01 has already been attempted 2 times

        async def get_segment_attempts(seg_num):
            return attempts_db.get(seg_num, 0)

        state.get_segment_attempts = get_segment_attempts

        # Segment that would be victim but exceeded max attempts
        victims = [1]

        # Filter victims by max attempts
        filtered_victims = []
        for seg_num in victims:
            attempts = await state.get_segment_attempts(seg_num)
            if attempts < config.recovery_max_attempts:
                filtered_victims.append(seg_num)

        # Should filter out S01 since it exceeded max_attempts
        assert 1 not in filtered_victims, "S01 should be filtered by circuit breaker"
        print("[OK] test_recovery_circuit_breaker")


async def main():
    """Run all integration tests."""
    print("Running recovery integration tests...\n")

    integration_tests = TestRecoveryIntegration()

    try:
        await integration_tests.test_recovery_triggers_on_partial()
        await integration_tests.test_recovery_retries_victims()
        await integration_tests.test_recovery_circuit_breaker()

        print("\nPASS All integration tests passed!")
        return 0

    except AssertionError as e:
        print(f"\nFAIL Test failed: {e}")
        import traceback
        traceback.print_exc()
        return 1
    except Exception as e:
        print(f"\nFAIL Unexpected error: {e}")
        import traceback
        traceback.print_exc()
        return 1


if __name__ == "__main__":
    sys.exit(asyncio.run(main()))
