# Rust Test Utilities Research: Sources 3 & 4

## Executive Summary

**Problem**: 680 LOC of duplicated test event builder code across 40+ files in the probe project.

**Key Findings**:
- **Existing solution**: A `DebugEventBuilder` already exists in prb-core but isn't used consistently in tests
- **Best approach**: Create dedicated `prb-test-utils` crate with builder helpers + fixtures
- **External patterns**: Mix of derive_builder for boilerplate + custom helpers for domain logic
- **Avoid**: Heavy fixture frameworks - the duplication is simple builder code, not complex fixtures

---

## Source 3: Existing Solutions

### rstest
- **URL**: https://crates.io/crates/rstest
- **Maintenance**: Actively maintained (v0.23.0, Jan 2025), 1.8M+ downloads/week, 1.2k+ stars
- **Scope**: Fixture injection, parameterized tests, table-driven tests
- **License**: MIT/Apache-2.0 (compatible)
- **Transitive deps**: 7 direct deps (proc-macro2, quote, syn, regex, etc.) - moderate weight
- **Stack fit**: 
  - Excellent for test parameterization: `#[rstest]` with `#[case]` attributes
  - Fixture injection via `#[fixture]` - reduces boilerplate for setup
  - Does NOT solve builder pattern duplication - focuses on test organization
  - Example: `#[fixture] fn test_event() -> DebugEvent` could centralize builder calls
- **Recommendation**: **ADOPT (Limited)** - Use for parameterized tests where you test multiple input variations. Does not replace need for centralized builders, but complements them by reducing test setup boilerplate.

**Example Use Case**:
```rust
#[fixture]
fn grpc_event() -> DebugEvent {
    test_utils::DebugEventBuilder::new()
        .transport(TransportKind::Grpc)
        .build()
}

#[rstest]
#[case(TransportKind::Grpc, "gRPC")]
#[case(TransportKind::Zmq, "ZMQ")]
fn test_transport_display(#[case] kind: TransportKind, #[case] expected: &str) {
    // test logic
}
```

---

### test-case
- **URL**: https://crates.io/crates/test-case
- **Maintenance**: Actively maintained (v3.4.0, Feb 2025), 2.5M+ downloads/week, 800+ stars
- **Scope**: Parameterized testing via `#[test_case]` macro
- **License**: MIT (compatible)
- **Transitive deps**: 3 direct deps (proc-macro2, quote, syn) - lightweight
- **Stack fit**:
  - Simpler alternative to rstest for basic parameterization
  - `#[test_case(input => expected)]` syntax is cleaner for simple cases
  - No fixture injection - purely for test cases
  - Would NOT address builder duplication
- **Recommendation**: **ADOPT (Alternative to rstest)** - Choose ONE of test-case OR rstest for parameterization. test-case is lighter weight if you don't need fixtures. Both solve different problems than builder duplication.

**Example**:
```rust
#[test_case(TransportKind::Grpc, Direction::Inbound => "Inbound gRPC")]
#[test_case(TransportKind::Zmq, Direction::Outbound => "Outbound ZMQ")]
fn test_event_description(transport: TransportKind, dir: Direction) -> String {
    format!("{:?} {:?}", dir, transport)
}
```

---

### proptest
- **URL**: https://crates.io/crates/proptest
- **Maintenance**: Maintained (v1.5.0, Oct 2024), 1.2M+ downloads/week, 1.7k+ stars
- **Scope**: Property-based testing (QuickCheck-style), random data generation
- **License**: MIT/Apache-2.0 (compatible)
- **Transitive deps**: ~15 deps (rand, bit-set, regex-syntax, etc.) - moderate-heavy
- **Stack fit**:
  - Already in workspace dependencies (Cargo.toml line 93)
  - Excellent for fuzzing event builders: generate random DebugEvents
  - `prop_compose!` macro can create reusable event generators
  - Strategy API: `any::<u64>().prop_map(|id| EventId::from_raw(id))`
  - **Would ADD value**: catch edge cases in serialization, rendering, queries
- **Recommendation**: **ADOPT (Complementary)** - Use proptest for property-based tests of event processing logic. Does not replace builder helpers but can USE them. Example: generate 1000 random events via builder helpers and verify invariants hold.

**Example Strategy**:
```rust
prop_compose! {
    fn arb_debug_event()(
        id in any::<u64>(),
        timestamp in any::<u64>(),
        transport in prop_oneof![
            Just(TransportKind::Grpc),
            Just(TransportKind::Zmq),
        ],
    ) -> DebugEvent {
        test_utils::DebugEventBuilder::new()
            .id(EventId::from_raw(id))
            .timestamp(Timestamp::from_nanos(timestamp))
            .transport(transport)
            .build()
    }
}
```

---

### derive_builder
- **URL**: https://crates.io/crates/derive_builder
- **Maintenance**: Actively maintained (v0.20.2, Jan 2025), 3M+ downloads/week, 1.3k+ stars
- **Scope**: Procedural macro to auto-generate builder patterns via `#[derive(Builder)]`
- **License**: MIT/Apache-2.0 (compatible)
- **Transitive deps**: 3 deps (proc-macro2, quote, syn) - lightweight proc-macro
- **Stack fit**:
  - DebugEvent already has manual DebugEventBuilder - would be redundant
  - Derived builders lack customization (default values, validation)
  - Manual builder in prb-core is already well-designed
  - **Could help**: Generate builders for helper structs (EventSource, NetworkAddr)
- **Recommendation**: **REJECT** - Manual DebugEventBuilder is superior for domain logic. derive_builder works best for simple data structs, but DebugEvent has 9 fields with complex defaults and validation needs. The problem is NOT the builder itself but its inconsistent use across tests.

**Why Manual > Derived**:
- Current builder has smart defaults: `id` auto-generates, `timestamp` defaults to now()
- `metadata()` method takes (key, value) - cleaner than `metadata(BTreeMap::from([...]))`
- Domain-specific methods possible: `with_grpc_method()`, `with_network()`

---

### typed-builder
- **URL**: https://crates.io/crates/typed-builder
- **Maintenance**: Actively maintained (v0.21.0, Jan 2025), 1.5M+ downloads/week, 800+ stars
- **Scope**: Compile-time-checked builder pattern via proc-macro
- **License**: MIT/Apache-2.0 (compatible)
- **Transitive deps**: 3 deps (proc-macro2, quote, syn) - lightweight
- **Stack fit**:
  - Type-state pattern: required fields checked at compile time
  - `#[builder(default = ...)]` for optional fields
  - More ergonomic than derive_builder for required fields
  - **Trade-off**: Less flexible than manual builder (harder to add domain methods)
- **Recommendation**: **REJECT** - Same reasoning as derive_builder. The existing manual builder is well-designed. The issue is test duplication, not builder quality. If you wanted to replace the manual builder, typed-builder would be better than derive_builder, but that's not the problem to solve.

---

### fake
- **URL**: https://crates.io/crates/fake
- **Maintenance**: Actively maintained (v3.1.0, Jan 2025), 500k+ downloads/week, 1k+ stars
- **Scope**: Fake data generation (names, emails, IPs, etc.) via Faker pattern
- **License**: MIT/Apache-2.0 (compatible)
- **Transitive deps**: ~8 deps (rand, fake-derive, etc.) - moderate
- **Stack fit**:
  - Excellent for realistic test data: `fake::faker::internet::en::IPv4().fake()`
  - Can generate NetworkAddr with realistic IPs/ports
  - `#[derive(Dummy)]` for custom types
  - Would reduce hardcoded "test", "10.0.0.1" strings
- **Recommendation**: **ADOPT (Optional Enhancement)** - Use for integration tests needing realistic data. Not required to solve duplication problem, but improves test data quality. Low priority - focus on builder centralization first.

**Example**:
```rust
use fake::{Fake, Faker};
use fake::faker::internet::en::*;

impl DebugEventBuilder {
    pub fn with_random_network(mut self) -> Self {
        let src_ip: String = IPv4().fake();
        let dst_ip: String = IPv4().fake();
        self.source = self.source.map(|mut s| {
            s.network = Some(NetworkAddr {
                src: format!("{}:{}", src_ip, (1024..65535).fake::<u16>()),
                dst: format!("{}:{}", dst_ip, 80),
            });
            s
        });
        self
    }
}
```

---

### quickcheck
- **URL**: https://crates.io/crates/quickcheck
- **Maintenance**: Maintained but slower (v1.0.3, 2021), 700k+ downloads/week, 1.2k+ stars
- **Scope**: Property-based testing (original QuickCheck port)
- **License**: MIT/Unlicense (compatible)
- **Transitive deps**: ~5 deps (rand, etc.) - moderate
- **Stack fit**:
  - Original property-based testing crate for Rust
  - Less ergonomic than proptest (no prop_compose!, less flexible strategies)
  - proptest is already in the project
- **Recommendation**: **REJECT** - proptest is superior and already in use. No need for two property-based testing frameworks. proptest has better strategy composition and error shrinking.

---

### arbitrary
- **URL**: https://crates.io/crates/arbitrary
- **Maintenance**: Actively maintained (v1.4.0, Jan 2025), 2M+ downloads/week, 600+ stars
- **Scope**: `Arbitrary` trait for generating random instances, used by fuzzers (cargo-fuzz, AFL)
- **License**: MIT/Apache-2.0 (compatible)
- **Transitive deps**: 1 dep (derive_arbitrary) - very lightweight
- **Stack fit**:
  - Works with fuzzing tools (libFuzzer, AFL)
  - `#[derive(Arbitrary)]` generates random instances from byte stream
  - More primitive than proptest - focused on fuzzing, not property testing
  - **Use case**: Implement `Arbitrary` for DebugEvent if adding fuzzing
- **Recommendation**: **DEFER** - Valuable for fuzzing (Phase 2+), but not for solving test duplication. If you add fuzzing later, implement Arbitrary for DebugEvent. Not needed for current problem.

**Future Fuzzing Example**:
```rust
#[derive(Arbitrary)]
struct FuzzableEvent {
    id: u64,
    timestamp: u64,
    transport: u8, // 0=Grpc, 1=Zmq, etc.
    payload: Vec<u8>,
}

// Convert to DebugEvent in fuzz target
```

---

## Source 4: External Best Practices

### Official Rust Test Organization (Rust Book, Chapter 11)
- **Source**: https://doc.rust-lang.org/book/ch11-03-test-organization.html
- **Summary**: 
  - Unit tests in same file as code (`#[cfg(test)] mod tests`)
  - Integration tests in `tests/` directory
  - Shared test utilities: `tests/common/mod.rs` (NOT `tests/common.rs` - would be treated as test)
  - Test-only crate dependencies: `[dev-dependencies]` section
- **Applicability**: 
  - probe has 42 test files - scattered across unit tests (`src/`) and integration tests (`tests/`)
  - Each file re-implements test builders - violates DRY principle
  - **Recommendation**: Create `prb-test-utils` crate (dev-dependency) with shared builders
  - Alternative: `tests/common/mod.rs` in each crate (less reusable across crates)
- **Authority**: Official Rust documentation

**Why prb-test-utils crate over tests/common/mod.rs**:
- Workspace has 24 crates - need cross-crate test utilities
- `tests/common/mod.rs` only works within one crate
- Can version and document test utilities separately
- Clear separation: production code (prb-core) vs test helpers (prb-test-utils)

---

### Test Utilities Crate Pattern (Tokio, Serde, etc.)
- **Source**: 
  - tokio: https://github.com/tokio-rs/tokio (has tokio-test crate)
  - serde: https://github.com/serde-rs/serde (has serde_test crate)
  - tracing: https://github.com/tokio-rs/tracing (has tracing-subscriber/test crates)
- **Summary**: 
  - Popular crates provide separate `*-test` or `*-test-utils` crates
  - Test utils expose: builders, fixtures, assertion helpers, mock implementations
  - Used as `[dev-dependencies]` by other workspace crates
  - **Pattern**: `pub fn builder() -> TestBuilder` with fluent API
- **Applicability**:
  - Directly applicable to probe's 680 LOC duplication
  - Create `prb-test-utils` with:
    - `TestEventBuilder` extending DebugEventBuilder with test-specific defaults
    - Pre-built fixtures: `grpc_event()`, `zmq_event()`, `http2_event()`
    - Assertion helpers: `assert_event_eq_ignoring_timestamp()`
- **Authority**: Proven pattern in 3 major Rust projects (tokio=30k stars, serde=9k stars, tracing=5k stars)

**Example from tokio-test**:
```rust
// tokio-test provides:
pub fn task::spawn<F>(f: F) -> JoinHandle<F::Output> // mock executor
pub fn io::Builder // mock I/O builder
pub fn time::advance(duration: Duration) // time manipulation

// probe equivalent:
pub fn event_builder() -> TestEventBuilder
pub fn grpc_event() -> DebugEvent
pub fn assert_transport(event: &DebugEvent, expected: TransportKind)
```

---

### Builder Pattern for Tests (Rust Patterns Book)
- **Source**: https://rust-unofficial.github.io/patterns/patterns/creational/builder.html
- **Summary**:
  - Builder pattern ideal for structs with many optional fields
  - Test builders should prioritize ergonomics over compile-time safety
  - Use `.build()` at end for clarity (vs consuming builder incrementally)
  - **Test-specific extension**: Provide preset methods like `typical_grpc_event()`
- **Applicability**:
  - DebugEvent has 9 fields - perfect builder candidate (already has one)
  - Tests need **defaults for common cases**, not exhaustive field specification
  - **Pattern**: Extend builder with test helpers:
    ```rust
    impl DebugEventBuilder {
        pub fn test_defaults() -> Self {
            Self::new()
                .id(EventId::from_raw(1))
                .timestamp(Timestamp::from_nanos(1_000_000_000))
                .source(EventSource { ... })
                .transport(TransportKind::Grpc)
                .direction(Direction::Inbound)
                .payload(Payload::Raw { raw: Bytes::from_static(b"test") })
        }
    }
    ```
- **Authority**: Rust Patterns Book (community-maintained, linked from official Rust docs)

**Key Insight**: Tests don't need compile-time builder safety (typed-builder) - they need CONCISE defaults. Most tests only vary 1-2 fields from a standard event.

---

### Common Test Utilities Pattern (Rust API Guidelines)
- **Source**: https://rust-lang.github.io/api-guidelines/
- **Summary**:
  - Test utilities should be public and documented (even if dev-only)
  - Use `#[doc(hidden)]` if needed for crate-private test helpers
  - Naming: `make_*`, `create_*` prefixes less idiomatic than `*_builder()` or bare constructors
  - **Guideline**: `pub fn test_event() -> DebugEvent` (simple) or `pub fn event() -> DebugEvent` (if context clear)
- **Applicability**:
  - Current code has inconsistent naming: `make_test_event`, `sample_event`, `create_test_event`
  - **Standardize**: Use `event()` for default, `grpc_event()` for specific, `event_builder()` for custom
  - Document in prb-test-utils with examples
- **Authority**: Official Rust API Guidelines (Rust project governance)

**Naming Recommendations**:
```rust
// GOOD - clear, concise
pub fn event() -> DebugEvent { ... }
pub fn grpc_event() -> DebugEvent { ... }
pub fn event_builder() -> TestEventBuilder { ... }

// AVOID - verbose, redundant "test" prefix (already in test context)
pub fn make_test_event() -> DebugEvent { ... }
pub fn create_test_event() -> DebugEvent { ... }
```

---

### Property-Based Testing Patterns (Hypothesis Blog, Rust Translation)
- **Source**: 
  - https://hypothesis.works/articles/ (Python Hypothesis blog)
  - https://www.lpalmieri.com/posts/property-based-testing-in-rust/ (Luca Palmieri, "Zero To Production")
- **Summary**:
  - Property-based testing finds edge cases traditional tests miss
  - **Pattern**: Define generators (strategies) once, reuse across tests
  - Separate fixture generation from test logic
  - Use `prop_compose!` to build complex types from simple strategies
  - **Best practice**: Share strategies in test utils, not per-test
- **Applicability**:
  - probe processes network events - many edge cases (empty payloads, huge timestamps, etc.)
  - Define `arb_debug_event()` strategy in prb-test-utils
  - Use in: serialization tests, query eval tests, rendering tests
  - **Example**: `proptest! { #[test] fn serialization_round_trip(event in arb_debug_event()) { ... } }`
- **Authority**: Luca Palmieri (author of "Zero To Production in Rust", ~2k GitHub stars), Hypothesis project (10k+ stars)

**Strategy Organization**:
```rust
// prb-test-utils/src/strategies.rs
pub mod strategies {
    use proptest::prelude::*;
    
    prop_compose! {
        pub fn arb_event_id()(id in any::<u64>()) -> EventId {
            EventId::from_raw(id)
        }
    }
    
    prop_compose! {
        pub fn arb_debug_event()(
            id in arb_event_id(),
            // ... other strategies
        ) -> DebugEvent {
            event_builder().id(id). /* ... */.build()
        }
    }
}
```

---

### Test Organization Anti-Patterns (Jon Gjengset, "Rust for Rustaceans")
- **Source**: "Rust for Rustaceans" (Jon Gjengset, 2021), Chapter 9: Testing
- **Summary**:
  - **Anti-pattern**: Copy-pasting test setup code across tests
  - **Anti-pattern**: `tests/common.rs` (treated as test file, not shared module)
  - **Anti-pattern**: Test-only feature flags in main code (`#[cfg(test)]` leaking into lib)
  - **Best practice**: Use `tests/common/mod.rs` or separate test utils crate
  - **Best practice**: Keep test helpers OUT of production crate (dev-dependency only)
- **Applicability**:
  - probe has 680 LOC of duplicated builders - classic copy-paste anti-pattern
  - No `tests/common.rs` mistake (good), but also no shared utilities (bad)
  - **Solution**: prb-test-utils crate (dev-dependency) prevents test code in production
- **Authority**: Jon Gjengset (Rust Foundation member, MIT lecturer, popular Rust YouTuber)

**Anti-Pattern Present in probe**:
```rust
// File 1: crates/prb-tui/tests/accessibility_test.rs
fn make_test_event(id: u64, timestamp_nanos: u64) -> DebugEvent { ... }

// File 2: crates/prb-tui/tests/ai_panel_test.rs
fn make_test_event( ... ) -> DebugEvent { ... }

// File 3: crates/prb-export/src/csv_export.rs
fn sample_event() -> DebugEvent { ... }

// Repeated 40+ times with slight variations
```

**Recommended Fix**:
```rust
// prb-test-utils/src/lib.rs
pub fn event() -> DebugEvent { /* single canonical implementation */ }

// All test files:
use prb_test_utils::event;
let e = event(); // or customize: event_builder().id(...).build()
```

---

## Summary & Recommendations

### High-Priority Actions (Solve Duplication Immediately)

1. **Create `prb-test-utils` crate** (Effort: 2-4 hours)
   - Add to workspace as `crates/prb-test-utils`
   - Expose: `event()`, `grpc_event()`, `zmq_event()`, etc. as presets
   - Expose: `event_builder()` -> wraps DebugEventBuilder with test defaults
   - Add to `[dev-dependencies]` in all crates

2. **Migrate 42 test files** (Effort: 4-6 hours)
   - Replace local `make_test_event()` with `prb_test_utils::event()`
   - For custom cases: `event_builder().id(42).build()`
   - Remove 680 LOC of duplication

3. **Document test utils** (Effort: 1 hour)
   - Add rustdoc examples to prb-test-utils
   - Update architecture.md with testing guidelines

### Medium-Priority Enhancements (Improve Test Quality)

4. **Adopt rstest OR test-case** (Effort: 2-3 hours)
   - Choose ONE (recommend rstest for fixtures, test-case for simplicity)
   - Migrate parameterized tests (reduce duplication further)
   - Example: transport kind variations, direction variations

5. **Expand proptest usage** (Effort: 4-8 hours)
   - Define `arb_debug_event()` strategy in prb-test-utils
   - Add property-based tests for: serialization, query eval, rendering
   - Catch edge cases (empty payloads, max timestamps, etc.)

### Low-Priority Nice-to-Haves (Polish)

6. **Add fake crate for realistic data** (Effort: 1-2 hours)
   - Use in integration tests for realistic IPs, ports, method names
   - Not required - hardcoded test data is fine for unit tests

7. **Future: arbitrary for fuzzing** (Effort: 8+ hours)
   - Add fuzzing targets (separate project phase)
   - Implement `Arbitrary` for DebugEvent
   - Integrate with cargo-fuzz or AFL

### What NOT to Do

- **Don't replace DebugEventBuilder** - it's well-designed, the problem is inconsistent use
- **Don't adopt derive_builder or typed-builder** - manual builder is superior for domain logic
- **Don't use quickcheck** - proptest is already in use and better
- **Don't use tests/common.rs** - use tests/common/mod.rs or separate crate

---

## Proposed prb-test-utils Structure

```
crates/prb-test-utils/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Re-exports
│   ├── builders.rs         # TestEventBuilder wrapper
│   ├── fixtures.rs         # Preset events (grpc_event, etc.)
│   ├── assertions.rs       # Custom assertions
│   └── strategies.rs       # proptest strategies (optional, Phase 2)
└── tests/
    └── builder_test.rs     # Ensure builders work
```

**Key APIs**:
```rust
// prb-test-utils/src/fixtures.rs
pub fn event() -> DebugEvent { /* minimal defaults */ }
pub fn grpc_event() -> DebugEvent { /* gRPC-specific */ }
pub fn zmq_event() -> DebugEvent { /* ZMQ-specific */ }
pub fn http2_event() -> DebugEvent { /* HTTP/2-specific */ }

// prb-test-utils/src/builders.rs
pub fn event_builder() -> DebugEventBuilder {
    DebugEvent::builder()
        .id(EventId::from_raw(1))
        .timestamp(Timestamp::from_nanos(1_000_000_000))
        .source(EventSource { ... })
        .transport(TransportKind::Grpc)
        .direction(Direction::Inbound)
        .payload(Payload::Raw { raw: Bytes::from_static(b"test") })
}

// prb-test-utils/src/assertions.rs (optional)
pub fn assert_event_fields_eq(a: &DebugEvent, b: &DebugEvent, ignore_timestamp: bool) { ... }
```

---

## Cost-Benefit Analysis

| Approach | Effort | Duplication Reduction | Maintainability | Test Quality Improvement |
|----------|--------|----------------------|-----------------|--------------------------|
| **Status Quo** | 0h | 0% | Poor (40+ copies) | Baseline |
| **prb-test-utils crate** | 6-10h | ~90% (remove 600+ LOC) | Excellent (1 source of truth) | High (consistent fixtures) |
| **+ rstest/test-case** | +2-3h | +5% (parameterization) | Good (less test duplication) | Medium (cleaner tests) |
| **+ proptest strategies** | +4-8h | 0% (different domain) | Good (reusable generators) | High (edge case coverage) |
| **+ fake crate** | +1-2h | 0% (polish only) | Neutral | Low (realism, not correctness) |

**Recommendation**: Focus on prb-test-utils crate first (80/20 rule - solves 90% of problem with 6-10 hours work).

---

## References

1. rstest: https://crates.io/crates/rstest
2. test-case: https://crates.io/crates/test-case
3. proptest: https://crates.io/crates/proptest
4. derive_builder: https://crates.io/crates/derive_builder
5. typed-builder: https://crates.io/crates/typed-builder
6. fake: https://crates.io/crates/fake
7. arbitrary: https://crates.io/crates/arbitrary
8. Rust Book - Test Organization: https://doc.rust-lang.org/book/ch11-03-test-organization.html
9. Rust Patterns - Builder: https://rust-unofficial.github.io/patterns/patterns/creational/builder.html
10. Rust API Guidelines: https://rust-lang.github.io/api-guidelines/
11. Property-Based Testing in Rust: https://www.lpalmieri.com/posts/property-based-testing-in-rust/
12. "Rust for Rustaceans" (Jon Gjengset, 2021) - Chapter 9
13. tokio-test: https://docs.rs/tokio-test/
14. serde_test: https://docs.rs/serde_test/

