---
segment: 05
title: prb-plugin-native to 85%
depends_on: []
risk: 5
complexity: High
cycle_budget: 12
estimated_lines: ~300 test lines
---

# Segment 05: prb-plugin-native Coverage to 85%

## Context

**Current:** 74.55%
**Target:** 85%
**Gap:** +10.45 percentage points

**CRITICAL GAP:**
- `src/loader.rs` - **3.83% (144 lines uncovered)** - dynamic library loading
- `src/adapter.rs` - 81.02% (100 lines uncovered) - plugin adapter

## Goal

Comprehensive tests for plugin loading, FFI safety, error handling.

## Exit Criteria

1. [ ] prb-plugin-native ≥85%
2. [ ] loader.rs ≥60% (realistic for FFI/unsafe code)
3. [ ] Test plugins with known behavior

## Implementation Plan

### Priority 1: Test Plugin Infrastructure (~150 lines)

Create mock test plugins:

```rust
// crates/prb-plugin-native/tests/fixtures/test_plugin.rs

#[no_mangle]
pub extern "C" fn prb_plugin_register() -> PluginMetadata {
    PluginMetadata {
        name: "test_plugin",
        version: "1.0.0",
    }
}
```

### Priority 2: Loader Tests (~150 lines)

```rust
// crates/prb-plugin-native/tests/loader_tests.rs

#[test]
fn test_load_valid_plugin() {
    let path = build_test_plugin();
    let result = PluginLoader::load(&path);
    assert!(result.is_ok());
}

#[test]
fn test_load_nonexistent_plugin() {
    let result = PluginLoader::load("nonexistent.so");
    assert!(matches!(result, Err(PluginError::LoadFailed(_))));
}

#[test]
fn test_load_invalid_binary() {
    // Test loading non-plugin .so file
}

#[test]
fn test_plugin_abi_mismatch() {
    // Test incompatible plugin version
}
```

## Test Plan

1. Build test plugin fixtures
2. Add loader integration tests
3. Test on Linux/macOS (Windows uses .dll)
4. Verify: `cargo test -p prb-plugin-native`

## Success Metrics

- prb-plugin-native: 74.55% → 85%+
- loader.rs: 3.83% → 60%+ (FFI is hard to fully test)
- ~40-50 new tests
