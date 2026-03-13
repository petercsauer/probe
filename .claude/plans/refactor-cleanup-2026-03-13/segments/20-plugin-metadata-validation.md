---
segment: 20
title: "Enhance Plugin Metadata Validation"
depends_on: [10]
risk: 3/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "feat(plugins): Add comprehensive plugin metadata validation"
---

# Segment 20: Enhance Plugin Metadata Validation

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Add validation for plugin metadata (version compatibility, required fields).

**Depends on:** Segment 10 (core APIs documented)

## Context: Issue 20 - Plugin Metadata Validation Gaps

**Core Problem:** Plugin loader accepts plugins without validating required metadata fields. Can load incompatible plugins.

## Scope
- **Files:** `crates/prb-plugin-native/src/loader.rs`, `crates/prb-plugin-wasm/src/loader.rs`

## Implementation Approach

Add validation checks:
```rust
fn validate_metadata(meta: &PluginMetadata) -> Result<(), PluginError> {
    if meta.name.is_empty() {
        return Err(PluginError::InvalidMetadata("name required"));
    }
    if meta.version.major > PLUGIN_API_VERSION {
        return Err(PluginError::IncompatibleVersion);
    }
    Ok(())
}
```

## Build and Test Commands

**Build:** `cargo build --package prb-plugin-native --package prb-plugin-wasm`

**Test (targeted):** `cargo test --package prb-plugin-native --lib loader`

**Test (regression):** `cargo test --workspace`

**Test (full gate):** `cargo test --workspace --all-targets`

## Exit Criteria

1. **Targeted tests:** Invalid plugins rejected with clear errors
2. **Regression tests:** All plugin tests pass
3. **Full build gate:** Clean build
4. **Full test suite:** All tests pass
5. **Self-review:** Validation comprehensive, error messages helpful
6. **Scope verification:** Only loader files modified
