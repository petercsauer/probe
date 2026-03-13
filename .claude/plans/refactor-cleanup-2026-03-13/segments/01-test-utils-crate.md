---
segment: 1
title: "Create Test Utilities Crate"
depends_on: []
risk: 3/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(test-utils): Create prb-test-utils crate with event fixtures and builders"
---

# Segment 1: Create Test Utilities Crate

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Create `prb-test-utils` crate with centralized test event builders and fixtures to eliminate 680 LOC of duplication across 42 test files.

**Depends on:** None

## Context: Issues Addressed

### Issue 01: Test Event Builder Duplication

**Core Problem:** 680 lines of duplicated test event builder code across 42 test files. Each file implements its own `make_test_event()`, `sample_event()`, or `create_test_event()` with nearly identical structure (10-29 lines per instance). Changes to `DebugEvent` structure require updating 42 locations.

**Proposed Fix:** Create `prb-test-utils` crate with:
- Fixture presets: `event()`, `grpc_event()`, `zmq_event()`, `http2_event()`, `dds_event()`
- Builder factory: `event_builder()` returning pre-configured `DebugEventBuilder` with test defaults
- Network helpers: `event_builder_with_network(src, dst)` for custom addresses

**Pre-Mortem Risks:**
- Migration breakage if tests have subtle fixture variations → Migrate conservatively, test after each file
- Dependency cycle if prb-core tests need prb-test-utils → Use dev-dependencies (allowed in workspaces)
- Feature drift if tests add custom variations → Provide flexible `event_builder()` for customization

## Scope

**Subsystem:** Testing infrastructure (test support layer)

**Crates affected:**
- NEW: `crates/prb-test-utils/` (create entire crate)
- Modified: `Cargo.toml` (workspace members, add prb-test-utils)
- Modified: 4 files as proof-of-concept migration (full migration is follow-up work):
  - `crates/prb-core/src/engine_tests.rs`
  - `crates/prb-export/src/csv_export.rs`
  - `crates/prb-export/src/html_export.rs`
  - `crates/prb-tui/tests/ai_panel_test.rs`

## Key Files and Context

### Existing DebugEventBuilder (to reuse)
- `/Users/psauer/probe/crates/prb-core/src/event.rs:300-400`
- Already has fluent builder API: `.id()`, `.timestamp()`, `.source()`, `.transport()`, etc.
- Returns `DebugEvent` on `.build()`

### Example of current duplication pattern
```rust
// crates/prb-core/src/engine_tests.rs:80-102
fn create_test_event(id: u64, timestamp_nanos: u64) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(timestamp_nanos),
        source: EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:1234".to_string(),
                dst: "10.0.0.2:5678".to_string(),
            }),
        },
        transport: TransportKind::Grpc,
        direction: Direction::Inbound,
        payload: Payload::Raw { raw: Bytes::from(vec![1, 2, 3]) },
        metadata: BTreeMap::new(),
        correlation_keys: vec![],
        sequence: None,
        warnings: vec![],
    }
}
```

### Project conventions for test organization
- ADR 0001: 20-crate workspace means test utilities need separate crate (not `tests/common/`)
- Existing pattern: `crates/prb-decode/tests/helpers/descriptor_builder.rs` uses builder pattern for test fixtures
- CONTRIBUTING.md line 109: Property tests with proptest for edge cases

### Proven pattern from external projects
- tokio-test (30k stars): Separate test utilities crate with fixtures
- serde_test (9k stars): `Token` builder, `assert_tokens` helpers
- tracing: `tracing-subscriber/src/testing/` - centralized test utilities

## Implementation Approach

### Step 1: Create crate structure
```
crates/prb-test-utils/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Public API and re-exports
│   ├── fixtures.rs     # Preset event factories
│   └── builders.rs     # Builder factories with test defaults
```

### Step 2: Implement fixtures.rs
```rust
use prb_core::{DebugEvent, DebugEventBuilder, EventId, Timestamp, /* ... */};

/// Creates a minimal test event with default values.
///
/// # Example
/// ```
/// use prb_test_utils::event;
/// let evt = event();
/// assert_eq!(evt.direction, Direction::Inbound);
/// ```
pub fn event() -> DebugEvent {
    event_builder().build()
}

/// Creates a gRPC test event with the given ID.
pub fn grpc_event(id: u64) -> DebugEvent {
    event_builder()
        .id(EventId::from_raw(id))
        .transport(TransportKind::Grpc)
        .build()
}

/// Creates a ZMQ test event with the given ID.
pub fn zmq_event(id: u64) -> DebugEvent {
    event_builder()
        .id(EventId::from_raw(id))
        .transport(TransportKind::Zmq)
        .direction(Direction::Outbound)
        .build()
}

// Similar for http2_event(), dds_event()
```

### Step 3: Implement builders.rs
```rust
use prb_core::{DebugEventBuilder, EventSource, NetworkAddr, Timestamp, Direction, Payload};
use bytes::Bytes;

/// Returns a pre-configured builder with test-friendly defaults.
///
/// Defaults:
/// - timestamp: 1_000_000_000 nanos (1970-01-01 00:00:01)
/// - source.adapter: "test"
/// - source.origin: "test"
/// - network: 10.0.0.1:1234 → 10.0.0.2:5678
/// - direction: Inbound
/// - payload: Raw(b"test")
///
/// # Example
/// ```
/// use prb_test_utils::event_builder;
/// let evt = event_builder()
///     .transport(TransportKind::Grpc)
///     .build();
/// ```
pub fn event_builder() -> DebugEventBuilder {
    DebugEventBuilder::new()
        .timestamp(Timestamp::from_nanos(1_000_000_000))
        .source(EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: "10.0.0.1:1234".to_string(),
                dst: "10.0.0.2:5678".to_string(),
            }),
        })
        .direction(Direction::Inbound)
        .payload(Payload::Raw {
            raw: Bytes::from(b"test".to_vec()),
        })
}

/// Returns a builder with custom network addresses.
pub fn event_builder_with_network(src: &str, dst: &str) -> DebugEventBuilder {
    DebugEventBuilder::new()
        .timestamp(Timestamp::from_nanos(1_000_000_000))
        .source(EventSource {
            adapter: "test".into(),
            origin: "test".into(),
            network: Some(NetworkAddr {
                src: src.to_string(),
                dst: dst.to_string(),
            }),
        })
        .direction(Direction::Inbound)
        .payload(Payload::Raw {
            raw: Bytes::from(b"test".to_vec()),
        })
}
```

### Step 4: Implement lib.rs
```rust
//! Test utilities for the probe project.
//!
//! This crate provides centralized test fixtures and builders for `DebugEvent`
//! to eliminate duplication across test files.
//!
//! # Examples
//!
//! ```
//! use prb_test_utils::{event, grpc_event, event_builder};
//! use prb_core::TransportKind;
//!
//! // Use a preset fixture
//! let evt = grpc_event(42);
//!
//! // Customize via builder
//! let custom = event_builder()
//!     .transport(TransportKind::Zmq)
//!     .build();
//! ```

mod fixtures;
mod builders;

pub use fixtures::*;
pub use builders::*;
```

### Step 5: Create Cargo.toml
```toml
[package]
name = "prb-test-utils"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
prb-core = { path = "../prb-core" }
bytes = { workspace = true }

[dev-dependencies]
# None needed - this is a test utilities crate
```

### Step 6: Add to workspace
Modify `/Users/psauer/probe/Cargo.toml`:
```toml
[workspace]
members = [
    # ... existing members ...
    "crates/prb-test-utils",
]
```

### Step 7: Migrate 4 files as proof-of-concept

**File 1: crates/prb-core/src/engine_tests.rs**
- **Before (lines 80-102):** 23-line `create_test_event()` function
- **After:**
  ```rust
  use prb_test_utils::event_builder;

  fn create_test_event(id: u64, timestamp_nanos: u64) -> DebugEvent {
      event_builder()
          .id(EventId::from_raw(id))
          .timestamp(Timestamp::from_nanos(timestamp_nanos))
          .build()
  }
  ```
  OR directly use: `event_builder().id(...).timestamp(...).build()`

**File 2: crates/prb-export/src/csv_export.rs**
- **Before (lines 166-185):** 20-line `sample_event()` function
- **After:**
  ```rust
  use prb_test_utils::event;

  fn sample_event() -> DebugEvent {
      event()
  }
  ```

**File 3: crates/prb-export/src/html_export.rs**
- Same as File 2 (duplicate of `sample_event()`)

**File 4: crates/prb-tui/tests/ai_panel_test.rs**
- **Before (lines 18-46):** 29-line `make_test_event()` function
- **After:**
  ```rust
  use prb_test_utils::event_builder;

  fn make_test_event(id: u64, timestamp_nanos: u64) -> DebugEvent {
      event_builder()
          .id(EventId::from_raw(id))
          .timestamp(Timestamp::from_nanos(timestamp_nanos))
          .build()
  }
  ```

### Step 8: Update dev-dependencies
Each migrated crate needs:
```toml
[dev-dependencies]
prb-test-utils = { path = "../prb-test-utils" }
```

Add to:
- `crates/prb-core/Cargo.toml`
- `crates/prb-export/Cargo.toml`
- `crates/prb-tui/Cargo.toml`

## Alternatives Ruled Out

1. **Use `tests/common/mod.rs` in each crate** - Rejected: Doesn't solve cross-crate duplication (would still have 15 implementations)
2. **Keep duplication, use macros** - Rejected: Reduces debuggability, doesn't establish single source of truth
3. **Add fixtures to prb-core** - Rejected: Violates separation of concerns (core types shouldn't know about test fixtures)

## Pre-Mortem Risks

1. **Dependency cycle**: prb-core tests need prb-test-utils which depends on prb-core
   - **Watch for**: `cargo build` errors about circular dependencies
   - **Test**: `cargo tree -p prb-test-utils` should show prb-core as dependency (not cycle)
   - **Mitigation**: Use in `[dev-dependencies]` only (allowed in workspaces)

2. **Migration breakage**: Some tests rely on subtle fixture variations
   - **Watch for**: Test failures after migration
   - **Test**: Run `cargo test --package <crate>` after each file migration
   - **Mitigation**: Migrate one file at a time, verify tests pass before next

3. **Import churn**: 4 files need new imports, potential for missing imports
   - **Watch for**: Compile errors about undefined types
   - **Test**: `cargo check --workspace` must pass
   - **Mitigation**: Use fully-qualified imports initially: `use prb_test_utils::event_builder;`

## Build and Test Commands

**Build:**
```bash
cargo build --package prb-test-utils
```

**Test (targeted):**
```bash
# Test the new crate itself (if we add examples/doctests)
cargo test --package prb-test-utils

# Test migrated files
cargo test --package prb-core --lib engine
cargo test --package prb-export --lib csv_export
cargo test --package prb-export --lib html_export
cargo test --package prb-tui --test ai_panel_test
```

**Test (regression):**
```bash
# All workspace tests must still pass
cargo test --workspace
```

**Test (full gate):**
```bash
cargo test --workspace --all-targets
```

## Exit Criteria

1. **Targeted tests:**
   - `cargo test --package prb-test-utils`: New crate compiles and doctests pass (if any)
   - `cargo test --package prb-core --lib engine`: Tests using `create_test_event()` pass
   - `cargo test --package prb-export`: Tests using `sample_event()` pass
   - `cargo test --package prb-tui --test ai_panel_test`: Tests using `make_test_event()` pass

2. **Regression tests:**
   - `cargo test --workspace`: All tests pass (no behavior changes)
   - No test timing regressions (fixture creation should be identical speed)

3. **Full build gate:**
   - `cargo build --workspace`: Clean build with no warnings
   - `cargo clippy --workspace --all-targets -- -D warnings`: No new clippy warnings

4. **Full test suite gate:**
   - `cargo test --workspace --all-targets`: All tests pass including integration tests
   - Coverage remains ≥80% (new utilities crate doesn't affect coverage - test code only)

5. **Self-review gate:**
   - No dead code: No unused `pub fn` in prb-test-utils
   - No commented-out blocks in migrated test files
   - No TODO hacks or workarounds
   - No out-of-scope changes (only create prb-test-utils + migrate 4 files, not all 42)

6. **Scope verification gate:**
   - **New files:**
     - `crates/prb-test-utils/Cargo.toml`
     - `crates/prb-test-utils/src/lib.rs`
     - `crates/prb-test-utils/src/fixtures.rs`
     - `crates/prb-test-utils/src/builders.rs`
   - **Modified files:**
     - `Cargo.toml` (workspace members)
     - `crates/prb-core/Cargo.toml` (dev-dependencies)
     - `crates/prb-core/src/engine_tests.rs` (use prb-test-utils)
     - `crates/prb-export/Cargo.toml` (dev-dependencies)
     - `crates/prb-export/src/csv_export.rs` (use prb-test-utils)
     - `crates/prb-export/src/html_export.rs` (use prb-test-utils)
     - `crates/prb-tui/Cargo.toml` (dev-dependencies)
     - `crates/prb-tui/tests/ai_panel_test.rs` (use prb-test-utils)
   - No other files modified
   - Remaining 38 test files NOT migrated yet (that's future work, not this segment)
