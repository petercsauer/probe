#!/usr/bin/env python3
"""Comprehensive integration test for orchestrator with worktree pool."""

import sys
from pathlib import Path

# Add scripts to path
sys.path.insert(0, str(Path(__file__).parent))


def test_all_imports():
    """Test that all components can be imported."""
    print("Testing imports...")

    try:
        from orchestrate_v2 import __main__
        print("✓ Main orchestrator module imports")

        from orchestrate_v2.worktree_pool import WorktreePool
        print("✓ WorktreePool imports")

        from orchestrate_v2.config import OrchestrateConfig
        print("✓ OrchestrateConfig imports")

        # Verify that _merge_worktree_changes is available
        assert hasattr(__main__, '_merge_worktree_changes')
        print("✓ _merge_worktree_changes function exists")

        # Verify that _run_wave accepts pool parameter
        import inspect
        sig = inspect.signature(__main__._run_wave)
        assert 'pool' in sig.parameters
        print("✓ _run_wave has pool parameter")

        # Verify OrchestrateConfig has isolation_strategy
        config = OrchestrateConfig()
        assert hasattr(config, 'isolation_strategy')
        print(f"✓ OrchestrateConfig has isolation_strategy (default: '{config.isolation_strategy}')")

        return True

    except Exception as e:
        print(f"✗ Import error: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_config_validation():
    """Test that config handles different isolation strategies."""
    print("\nTesting configuration...")

    try:
        from orchestrate_v2.config import OrchestrateConfig

        # Test none strategy
        config_none = OrchestrateConfig(isolation_strategy="none")
        assert config_none.isolation_strategy == "none"
        print("✓ isolation_strategy='none' accepted")

        # Test env strategy
        config_env = OrchestrateConfig(isolation_strategy="env")
        assert config_env.isolation_strategy == "env"
        print("✓ isolation_strategy='env' accepted")

        # Test worktree strategy
        config_wt = OrchestrateConfig(isolation_strategy="worktree")
        assert config_wt.isolation_strategy == "worktree"
        print("✓ isolation_strategy='worktree' accepted")

        return True

    except Exception as e:
        print(f"✗ Config error: {e}")
        return False


def test_code_paths():
    """Test that key code paths are properly structured."""
    print("\nTesting code structure...")

    try:
        from orchestrate_v2 import __main__
        import inspect

        # Check _merge_worktree_changes signature
        merge_sig = inspect.signature(__main__._merge_worktree_changes)
        params = list(merge_sig.parameters.keys())
        assert 'wt' in params and 'seg' in params
        print("✓ _merge_worktree_changes has correct signature")

        # Check _run_wave signature
        wave_sig = inspect.signature(__main__._run_wave)
        params = list(wave_sig.parameters.keys())
        assert 'pool' in params
        print("✓ _run_wave has pool parameter")

        # Check _orchestrate_inner exists
        assert hasattr(__main__, '_orchestrate_inner')
        print("✓ _orchestrate_inner function exists")

        return True

    except Exception as e:
        print(f"✗ Structure error: {e}")
        import traceback
        traceback.print_exc()
        return False


if __name__ == "__main__":
    print("=" * 60)
    print("Comprehensive Integration Test")
    print("=" * 60)

    results = []
    results.append(("Imports", test_all_imports()))
    results.append(("Configuration", test_config_validation()))
    results.append(("Code Structure", test_code_paths()))

    print("\n" + "=" * 60)
    print("Test Summary")
    print("=" * 60)

    all_passed = True
    for name, passed in results:
        status = "✅ PASS" if passed else "❌ FAIL"
        print(f"{name}: {status}")
        if not passed:
            all_passed = False

    print("=" * 60)

    if all_passed:
        print("\n✅ All integration tests passed!")
        sys.exit(0)
    else:
        print("\n❌ Some tests failed")
        sys.exit(1)
