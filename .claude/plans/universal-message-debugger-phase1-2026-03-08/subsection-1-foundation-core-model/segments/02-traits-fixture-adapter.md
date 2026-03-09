---
segment: 2
title: "Core Traits + JSON Fixture Adapter"
depends_on: [1]
risk: 3/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(core): add extension traits and JSON fixture adapter"
---

# Segment 2: Core Traits + JSON Fixture Adapter

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Define all 5 core extension traits in prb-core and implement the first CaptureAdapter (JSON fixture adapter) in a new prb-fixture crate.

**Depends on:** Segment 1

## Context: Issues Addressed

**S1-2 (sync trait design):** The parent plan did not address sync vs async for the 5 core traits. Async fn in traits is not dyn-compatible (cannot use Box<dyn CaptureAdapter>). Make all 5 traits synchronous for Phase 1 -- offline analysis only, all I/O is blocking file reads. CaptureAdapter uses Box<dyn Iterator> for streaming. Pre-mortem: Phase 2 live capture may require async traits (sync traits remain valid; async variants are additive); Box<dyn Iterator> may limit optimizations (concrete types can bypass for hot paths); large captures may block (acceptable for Phase 1 CLI).

**S1-4 (fixture format):** The parent plan did not specify the JSON fixture file format. Define versioned format: version field, description (optional), events array. Each event: timestamp_ns (nanoseconds), transport (snake_case), direction (inbound/outbound), exactly one of payload_base64 or payload_utf8, optional metadata (dotted keys), optional source (network addresses). Pre-mortem: base64 error-prone for hand-authored fixtures (provide payload_utf8 alternative); format may need new fields (version enables evolution); use serde_json strict parsing with clear error messages.

**S1-5 (error convention):** FixtureError in prb-fixture: Io (path + source), Parse, UnsupportedVersion, Core (via #[from]). Conversion chain: CoreError -> FixtureError -> anyhow::Error at CLI boundary. Pre-mortem: FixtureError::Parse should include event index and field name for helpful messages; empty fixtures (zero events) should succeed, not error.

## Scope

- `crates/prb-core/src/traits.rs` (5 trait definitions + supporting types)
- `crates/prb-core/src/decode.rs`, `flow.rs`, `schema.rs` (supporting types)
- `crates/prb-fixture/` (new crate: JSON fixture adapter)
- `fixtures/` (additional test fixtures)

## Key Files and Context

After Segment 1, the following exist:
- `crates/prb-core/src/event.rs` -- DebugEvent, TransportKind, Direction, EventSource, NetworkAddr, Payload, CorrelationKey, Timestamp, EventId
- `crates/prb-core/src/error.rs` -- CoreError with #[non_exhaustive]
- `crates/prb-core/src/lib.rs` -- re-exports

Files to create:
- `crates/prb-core/src/traits.rs` -- CaptureAdapter, ProtocolDecoder, SchemaResolver, EventNormalizer, CorrelationStrategy traits
- `crates/prb-core/src/decode.rs` -- DecodeContext and DecodedPayload supporting types for ProtocolDecoder
- `crates/prb-core/src/flow.rs` -- Flow type for CorrelationStrategy return value
- `crates/prb-core/src/schema.rs` -- ResolvedSchema type for SchemaResolver return value
- `crates/prb-fixture/Cargo.toml` -- depends on prb-core, serde, serde_json, thiserror, camino, bytes
- `crates/prb-fixture/src/lib.rs` -- re-exports
- `crates/prb-fixture/src/adapter.rs` -- JsonFixtureAdapter implementing CaptureAdapter
- `crates/prb-fixture/src/format.rs` -- FixtureFile, FixtureEvent serde types (the JSON schema)
- `crates/prb-fixture/src/error.rs` -- FixtureError enum
- `fixtures/grpc_sample.json` -- gRPC fixture with base64 payload
- `fixtures/multi_transport.json` -- mixed transport types
- `fixtures/empty.json` -- empty events array
- `fixtures/malformed.json` -- intentionally invalid for error testing

Trait definitions (all sync, in `crates/prb-core/src/traits.rs`):

```rust
pub trait CaptureAdapter {
    fn name(&self) -> &str;
    fn ingest(&mut self) -> Box<dyn Iterator<Item = Result<DebugEvent, CoreError>> + '_>;
}

pub trait ProtocolDecoder {
    fn protocol(&self) -> TransportKind;
    fn decode_stream(&mut self, data: &[u8], ctx: &DecodeContext) -> Result<Vec<DebugEvent>, CoreError>;
}

pub trait SchemaResolver {
    fn resolve(&self, schema_name: &str) -> Result<Option<ResolvedSchema>, CoreError>;
    fn list_schemas(&self) -> Vec<String>;
}

pub trait EventNormalizer {
    fn normalize(&self, events: Vec<DebugEvent>) -> Result<Vec<DebugEvent>, CoreError>;
}

pub trait CorrelationStrategy {
    fn transport(&self) -> TransportKind;
    fn correlate<'a>(&self, events: &'a [DebugEvent]) -> Result<Vec<Flow<'a>>, CoreError>;
}
```

JSON fixture format (in `crates/prb-fixture/src/format.rs`):
```rust
#[derive(Debug, Deserialize)]
pub struct FixtureFile {
    pub version: u64,
    #[serde(default)]
    pub description: Option<String>,
    pub events: Vec<FixtureEvent>,
}

#[derive(Debug, Deserialize)]
pub struct FixtureEvent {
    pub timestamp_ns: u64,
    pub transport: String,
    #[serde(default = "default_direction")]
    pub direction: String,
    pub payload_base64: Option<String>,
    pub payload_utf8: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    pub source: Option<FixtureSource>,
}
```

JsonFixtureAdapter: constructor takes `Utf8PathBuf`, `ingest()` reads file, parses JSON, converts each FixtureEvent to DebugEvent, yields via Iterator. Validation: exactly one of payload_base64 or payload_utf8 must be present; version must be 1; transport string must map to valid TransportKind.

## Implementation Approach

1. Add `traits.rs`, `decode.rs`, `flow.rs`, `schema.rs` to prb-core
2. Update `prb-core/src/lib.rs` to re-export traits and supporting types
3. Create `crates/prb-fixture/` with Cargo.toml inheriting workspace deps
4. Implement FixtureFile/FixtureEvent serde types in `format.rs`
5. Implement FixtureError in `error.rs`
6. Implement JsonFixtureAdapter in `adapter.rs`
7. Add prb-fixture to workspace members in root Cargo.toml
8. Create test fixtures in `fixtures/`
9. Write comprehensive tests in prb-fixture

## Alternatives Ruled Out

- Async traits: not dyn-safe, no async I/O needed for Phase 1.
- Stream instead of Iterator: adds tokio dependency for no benefit.
- YAML fixture format: adds dependency, JSON is sufficient.
- Newline-delimited JSON: harder to hand-edit.
- Single payload field with auto-detection: ambiguous; explicit payload_base64 vs payload_utf8 is clearer.

## Pre-Mortem Risks

- Trait signatures may need revision when Subsection 2-5 implement them. Use only types from prb-core; #[non_exhaustive] on error enums; supporting types are intentionally minimal.
- Base64 decoding errors may produce unhelpful messages. FixtureError::Parse includes event index and field name.
- Empty fixtures (zero events) should succeed. Test explicitly.

## Build and Test Commands

- Build: `cargo build -p prb-core && cargo build -p prb-fixture`
- Test (targeted): `cargo nextest run -p prb-core -p prb-fixture`
- Test (regression): `cargo nextest run -p prb-core`
- Test (full gate): `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings && cargo fmt --all -- --check`

## Exit Criteria

1. **Targeted tests:**
   - `test_capture_adapter_trait_object_safe`: verify CaptureAdapter can be used as `Box<dyn CaptureAdapter>`
   - `test_fixture_parse_grpc_sample`: parse `fixtures/grpc_sample.json`, verify correct number of events and transport type
   - `test_fixture_parse_multi_transport`: parse `fixtures/multi_transport.json`, verify mixed transport types are correct
   - `test_fixture_parse_empty`: parse `fixtures/empty.json` with zero events, verify success with empty iterator
   - `test_fixture_parse_malformed`: parse `fixtures/malformed.json`, verify FixtureError::Parse is returned
   - `test_fixture_unsupported_version`: fixture with `"version": 99`, verify FixtureError::UnsupportedVersion
   - `test_fixture_missing_payload`: fixture event with neither payload_base64 nor payload_utf8, verify error
   - `test_fixture_both_payloads`: fixture event with both payload fields, verify error
   - `test_fixture_base64_decode`: verify binary payload round-trips through base64
   - `test_fixture_utf8_payload`: verify UTF-8 payload stored as Bytes in DebugEvent
   - `test_fixture_metadata_preserved`: verify metadata key-value pairs pass through to DebugEvent.metadata
   - `test_fixture_network_addr`: verify source with network addresses populates EventSource.network
   - `test_fixture_adapter_name`: verify `name()` returns `"json-fixture"`
   - `test_fixture_io_error_nonexistent_file`: verify FixtureError::Io for missing file
   - proptest: `test_fixture_event_arbitrary` -- generate random FixtureEvents, verify they convert to valid DebugEvents without panicking
2. **Regression tests:** All Segment 1 tests pass (`cargo nextest run -p prb-core`)
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo nextest run --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changed files are limited to: `Cargo.toml` (workspace members), `Cargo.lock`, `crates/prb-core/src/traits.rs`, `crates/prb-core/src/decode.rs`, `crates/prb-core/src/flow.rs`, `crates/prb-core/src/schema.rs`, `crates/prb-core/src/lib.rs`, `crates/prb-fixture/**`, `fixtures/**`.
