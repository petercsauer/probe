---
id: "01"
title: "Test Event Builder Duplication"
risk: 3/10
addressed_by_segments: [1]
---

# Issue 01: Test Event Builder Duplication

## Core Problem

680 lines of duplicated test event builder code across 42 test files. Each file implements its own variant of `make_test_event()`, `sample_event()`, or `create_test_event()` with nearly identical structure (10-29 lines per instance). Changes to `DebugEvent` structure require updating 42 locations. No single source of truth for test fixtures.

**Evidence:**
- `crates/prb-export/src/csv_export.rs:166-185` (20 lines, `sample_event`)
- `crates/prb-export/src/html_export.rs:402-421` (20 lines, `sample_event`)
- `crates/prb-core/src/engine_tests.rs:80-102` (23 lines, `create_test_event`)
- `crates/prb-tui/tests/ai_panel_test.rs:18-46` (29 lines, `make_test_event`)
- ...38 more files with similar patterns

## Root Cause

No centralized test utilities crate. Historical pattern of copy-pasting test setup code. `DebugEventBuilder` exists in `prb-core` but is inconsistently used in tests - some tests use it, most don't. No project-wide convention for test fixture creation.

## Proposed Fix

Create `prb-test-utils` crate with:
1. **Fixture presets**: `event()`, `grpc_event()`, `zmq_event()`, `http2_event()`, `dds_event()` - protocol-specific defaults
2. **Builder factory**: `event_builder()` - returns pre-configured `DebugEventBuilder` with test-friendly defaults
3. **Network helpers**: `event_builder_with_network(src, dst)` - custom addresses
4. **Proptest strategies** (Phase 2): Centralized property-based test generators

Migrate 42 test files to use centralized utilities. Each migration reduces local test setup from 10-29 lines to 1-3 lines.

**Implementation:**
```rust
// crates/prb-test-utils/src/fixtures.rs
pub fn event() -> DebugEvent {
    event_builder().build()
}

pub fn grpc_event(id: u64) -> DebugEvent {
    event_builder()
        .id(EventId::from_raw(id))
        .transport(TransportKind::Grpc)
        .build()
}

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
}
```

## Existing Solutions Evaluated

### rstest (crates.io/crates/rstest)
- **Maintenance**: Active (v0.23.0, 2024)
- **Scope**: Fixture injection, parameterized tests
- **License**: MIT/Apache-2.0
- **Stack fit**: Complements builder pattern, doesn't replace it
- **Recommendation**: **ADOPT (Phase 2)** - Use for parameterized test cases after establishing base fixtures

### test-case (crates.io/crates/test-case)
- **Maintenance**: Active (v3.3.1, 2024)
- **Scope**: Lightweight parameterized tests (alternative to rstest)
- **Recommendation**: **DEFER** - rstest has more features, stick with one solution

### proptest (already in project)
- **Status**: Already used in `navigation_property_test.rs`
- **Recommendation**: **ADOPT** - Centralize strategies in prb-test-utils (Phase 2)

### fake (crates.io/crates/fake)
- **Scope**: Realistic test data generation (IPs, ports, timestamps)
- **Recommendation**: **DEFER** - Current hard-coded defaults sufficient for unit tests

### derive_builder (crates.io/crates/derive_builder)
- **Recommendation**: **REJECT** - Manual `DebugEventBuilder` in prb-core is already well-designed, adding derive macro adds complexity without benefit

## Alternatives Considered

### Alternative 1: `tests/common/mod.rs` in each crate
- **Rejected**: ADR 0001 established 20-crate workspace, common pattern is per-crate `tests/helpers/`. But test event building is needed across 15 crates - would still have 15 implementations.
- **Why rejected**: Doesn't solve cross-crate duplication, only intra-crate

### Alternative 2: Keep duplication, use macros
- **Rejected**: Macros reduce debuggability, harder to understand for contributors
- **Why rejected**: Doesn't establish single source of truth, maintenance burden remains

### Alternative 3: Add fixtures to prb-core
- **Rejected**: Violates separation of concerns (core types shouldn't know about test fixtures)
- **Why rejected**: prb-core is production code, test utilities belong in separate crate

## Pre-Mortem — What Could Go Wrong

1. **Migration breakage**: Some tests have subtle variations in fixture setup
   - **Mitigation**: Migrate one file at a time, run full test suite after each
   - **Detection**: `cargo test --workspace` must pass after each migration

2. **Feature drift**: Tests add custom fixture variations, defeating centralization
   - **Mitigation**: Provide flexible `event_builder()` that returns builder for customization
   - **Prevention**: Document pattern in CONTRIBUTING.md

3. **Dependency cycle**: prb-test-utils depends on prb-core, what if prb-core tests need it?
   - **Mitigation**: prb-core tests can use prb-test-utils in dev-dependencies (allowed in workspaces)
   - **Validation**: Check with `cargo tree` after implementation

4. **Import churn**: 42 files need new imports
   - **Mitigation**: Semi-automated with search-replace, verify with `cargo check --workspace`
   - **Estimate**: ~10 minutes per file, 7 hours total

## Risk Factor

**3/10** - Low-moderate risk
- Changes are localized to test code (no production impact)
- Each migration is independently verifiable (test suite passes = success)
- Reversible (can revert individual test file migrations)
- High confidence (pattern proven by tokio-test, serde_test, tracing)

## Evidence for Optimality

### Source 1 (Codebase Evidence)
- `prb-core/src/event.rs:300-400`: `DebugEventBuilder` already exists and is well-designed
- `crates/prb-decode/tests/helpers/descriptor_builder.rs`: Project already uses helper pattern for protobuf test fixtures
- Pattern of 42 files with 10-29 line duplication (grep results)

### Source 2 (Project Conventions)
- CONTRIBUTING.md line 109: "Use property tests with proptest for parser edge cases"
- ADR 0001: 20-crate workspace means test utilities must be in separate crate (not tests/common/)
- `.claude/commands/iterative-builder.md:151`: "Never run cargo from crate subdirs" - workspace pattern established

### Source 3 (Existing Solutions)
- **tokio-test** (30k stars): Separate test utilities crate with fixtures and builders
- **serde_test** (9k stars): Same pattern - `Token` builder, `assert_tokens` helpers
- **tracing** (5k stars): `tracing-subscriber/src/testing/` - centralized test utilities

### Source 4 (External Best Practices)
- Rust API Guidelines (rust-lang.github.io/api-guidelines): "Avoid test code in production crates"
- Jon Gjengset "Rust for Rustaceans" Ch. 9: Anti-pattern - "Copy-paste test setup" (probe has 680 LOC of this)
- Luca Palmieri "Zero to Production" Ch. 7: "Centralize test fixtures early, before they proliferate"

## Blast Radius

### Direct Changes
- **New files**:
  - `crates/prb-test-utils/Cargo.toml`
  - `crates/prb-test-utils/src/lib.rs`
  - `crates/prb-test-utils/src/fixtures.rs`
  - `crates/prb-test-utils/src/builders.rs`
- **Modified**: `Cargo.toml` (workspace members list)
- **Modified**: 42 test files (imports + function call changes)

### Potential Ripple
- None - test utilities don't affect production code paths
- No API changes to `DebugEvent` or `DebugEventBuilder`
- No transitive dependency changes (prb-test-utils only in dev-dependencies)

### Test Impact
- All existing tests continue to work (just call centralized functions)
- Test execution time unchanged (same fixture creation, different location)
- Coverage unchanged (test logic identical, setup centralized)

### Documentation Impact
- Add section to CONTRIBUTING.md: "Writing Tests" with prb-test-utils examples
- Update docs/architecture.md: Add prb-test-utils to crate map (test support layer)
