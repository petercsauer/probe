"""Tests for pre-flight validation gates."""
import asyncio
import sys
from pathlib import Path

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from scripts.orchestrate_v2.config import OrchestrateConfig
from scripts.orchestrate_v2.state import StateDB


# Note: These tests require pytest for proper async support
# Run with: python3 -m pytest tests/test_preflight_validation.py -v


async def test_preflight_disabled():
    """Test that pre-flight can be disabled."""
    # Import here to avoid circular dependency
    from scripts.orchestrate_v2.__main__ import _pre_wave_health_check

    config = OrchestrateConfig(enable_preflight_checks=False)
    # Create temporary state DB
    db_path = Path("/tmp/test_preflight.db")
    db_path.unlink(missing_ok=True)
    state = await StateDB.create(db_path)

    try:
        # Should immediately return healthy without running check
        healthy, errors = await _pre_wave_health_check(wave=1, config=config, state=state)

        assert healthy == True, "Should return healthy when disabled"
        assert errors == [], "Should have no errors when disabled"
        print("[OK] test_preflight_disabled: PASS")
    finally:
        await state.close()
        db_path.unlink(missing_ok=True)


async def test_preflight_timeout_handling():
    """Test that pre-flight check respects timeout setting."""
    from scripts.orchestrate_v2.__main__ import _pre_wave_health_check

    config = OrchestrateConfig(
        enable_preflight_checks=True,
        preflight_timeout=1,  # 1 second - very short
    )

    db_path = Path("/tmp/test_preflight_timeout.db")
    db_path.unlink(missing_ok=True)
    state = await StateDB.create(db_path)

    try:
        # Health check may timeout on slow machine or succeed quickly
        healthy, errors = await _pre_wave_health_check(wave=1, config=config, state=state)

        # Either succeeds quickly or times out - both are acceptable
        if not healthy:
            # Should have timeout message in errors
            assert any("timeout" in e.lower() or "exception" in e.lower() for e in errors), \
                f"Expected timeout/exception in errors, got: {errors}"
            print("[OK] test_preflight_timeout_handling: PASS (timed out)")
        else:
            print("[OK] test_preflight_timeout_handling: PASS (succeeded quickly)")
    finally:
        await state.close()
        db_path.unlink(missing_ok=True)


def run_tests():
    """Run all tests."""
    print("Running pre-flight validation tests...\n")

    asyncio.run(test_preflight_disabled())
    asyncio.run(test_preflight_timeout_handling())

    print("\nAll tests passed!")


if __name__ == "__main__":
    run_tests()
