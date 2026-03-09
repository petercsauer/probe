---
segment: 1
title: "Workspace Skeleton + Core Types + Error Convention"
depends_on: []
risk: 2/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "feat(core): add workspace skeleton, DebugEvent model, and error types"
---

# Segment 1: Workspace Skeleton + Core Types + Error Convention

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Create the Cargo workspace with the core types crate containing DebugEvent, all supporting types, and the error convention.

**Depends on:** None

## Context: Issues Addressed

**S1-1 (DebugEvent model):** The DebugEvent type is the single most critical type in the project -- every crate and subsection produces, transforms, or consumes it. The parent plan defined zero fields. Define DebugEvent with fields informed by MCAP, Wireshark, and CloudEvents: id, timestamp, source, transport, direction, payload, metadata, correlation_keys, sequence, warnings. Supporting types: EventId (monotonic u64), Timestamp (nanoseconds), EventSource, NetworkAddr, TransportKind, Direction, Payload (Raw/Decoded), CorrelationKey. Define well-known metadata key constants. Pre-mortem: metadata BTreeMap may be too loose (use constants); Payload::Decoded.fields as serde_json::Value may be heavyweight (benchmark later); use #[serde(default)] on optional fields for forward compatibility.

**S1-3 (workspace structure):** The parent plan did not define crate names or directory layout. Create the first 3 crates: prb-core, prb-fixture, prb-cli. Use virtual workspace manifest, workspace.dependencies, crates/ directory, edition 2024. Pre-mortem: 14 crates may increase compile times (mitigation: incremental builds); workspace-level tests live in CLI crate.

**S1-5 (error convention):** Parent plan defined thiserror for libs, anyhow for CLI, but not concrete types. Define CoreError in prb-core with InvalidTimestamp, PayloadDecode, UnsupportedTransport, Serialization variants. Use #[non_exhaustive]. Pre-mortem: error variants may need changes (thiserror enums are additive); use #[source] to preserve error chains.

## Scope

- Root workspace manifest
- `crates/prb-core/` crate (types + errors)
- `crates/prb-cli/` minimal binary stub (just enough to compile)
- `fixtures/sample.json` (one example fixture)

## Key Files and Context

Files to create:
- `Cargo.toml` -- virtual workspace manifest, edition 2024, resolver 3
- `crates/prb-core/Cargo.toml` -- lib crate
- `crates/prb-core/src/lib.rs` -- re-exports
- `crates/prb-core/src/event.rs` -- DebugEvent and all supporting types
- `crates/prb-core/src/error.rs` -- CoreError enum
- `crates/prb-cli/Cargo.toml` -- binary crate (depends on prb-core)
- `crates/prb-cli/src/main.rs` -- minimal `fn main()` that compiles

The DebugEvent type must carry:
- `id: EventId` -- newtype over u64 (monotonic counter)
- `timestamp: Timestamp` -- newtype over u64 (nanoseconds since Unix epoch)
- `source: EventSource` -- struct with `adapter: String`, `origin: String`, `network: Option<NetworkAddr>`
- `transport: TransportKind` -- enum { Grpc, Zmq, DdsRtps, RawTcp, RawUdp, JsonFixture }
- `direction: Direction` -- enum { Inbound, Outbound, Unknown }
- `payload: Payload` -- enum { Raw(Bytes), Decoded { raw: Bytes, fields: serde_json::Value, schema_name: Option<String> } }
- `metadata: BTreeMap<String, String>` -- protocol-specific key-value pairs
- `correlation_keys: Vec<CorrelationKey>` -- enum { StreamId(u32), Topic(String), ConnectionId(String), Custom(String, String) }
- `sequence: Option<u64>` -- ordering within a stream
- `warnings: Vec<String>` -- parse warnings (per no-ignore-failure rule)

Well-known metadata key constants:
```rust
pub const METADATA_KEY_GRPC_METHOD: &str = "grpc.method";
pub const METADATA_KEY_H2_STREAM_ID: &str = "h2.stream_id";
pub const METADATA_KEY_ZMQ_TOPIC: &str = "zmq.topic";
pub const METADATA_KEY_DDS_DOMAIN_ID: &str = "dds.domain_id";
pub const METADATA_KEY_DDS_TOPIC_NAME: &str = "dds.topic_name";
```

Workspace dependencies:
```toml
[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
bytes = { version = "1", features = ["serde"] }
thiserror = "2"
anyhow = "1"
clap = { version = "4", features = ["derive"] }
camino = { version = "1.2", features = ["serde1"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
tokio = { version = "1", features = ["full"] }
```

Testing dependencies:
```toml
insta = { version = "1", features = ["json", "yaml"] }
proptest = "1"
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

All types must derive `Debug, Clone, PartialEq, Serialize, Deserialize`. TransportKind and Direction must derive `Copy`. DebugEvent must use `#[serde(tag = "type")]` for internal tagging when serialized to JSON. CoreError must be `#[non_exhaustive]`.

## Implementation Approach

1. Create root `Cargo.toml` as virtual workspace with `workspace.dependencies`
2. Create `crates/prb-core/` with Cargo.toml inheriting workspace deps
3. Implement all types in `event.rs` with full serde derives
4. Implement `CoreError` in `error.rs` with thiserror
5. Create `crates/prb-cli/` with minimal main.rs
6. Write unit tests: serde round-trip for every type, Display impls, error chain verification
7. Create `fixtures/sample.json` with one valid fixture matching the format spec (version 1, events array with at least one event)

## Alternatives Ruled Out

- UUID for EventId: 16 bytes overhead per event; monotonic u64 is sufficient and cheaper.
- `serde_json::Value` as the entire event model: loses compile-time guarantees.
- Protobuf `DynamicMessage` as event model: circular dependency with the decode engine.
- `anyhow` in library crates: violates the error convention; library consumers can't match on specific variants.

## Pre-Mortem Risks

- DebugEvent field set may be incomplete for later subsections. Use `#[serde(default)]` on optional fields, `#[non_exhaustive]` on enums.
- `serde_json::Value` in Payload::Decoded may be slow for high-throughput. Acceptable for Phase 1; Subsection 5 benchmarks will validate.
- `BTreeMap<String, String>` for metadata doesn't enforce well-known keys at compile time. The constant definitions mitigate this.

## Build and Test Commands

- Build: `cargo build -p prb-core && cargo build -p prb-cli`
- Test (targeted): `cargo nextest run -p prb-core`
- Test (regression): N/A (first segment, no prior code)
- Test (full gate): `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings && cargo fmt --all -- --check`

## Exit Criteria

1. **Targeted tests:**
   - `test_debug_event_serde_roundtrip`: serialize DebugEvent to JSON and deserialize back, assert equality
   - `test_timestamp_nanosecond_precision`: verify Timestamp preserves nanosecond values
   - `test_payload_raw_serde`: verify Raw payload base64-encodes/decodes correctly with serde
   - `test_payload_decoded_serde`: verify Decoded payload round-trips with fields and schema_name
   - `test_transport_kind_display`: verify Display impl for all TransportKind variants
   - `test_direction_display`: verify Display impl for all Direction variants
   - `test_core_error_display`: verify CoreError variants produce meaningful messages
   - `test_core_error_source_chain`: verify #[source] chains are preserved
   - `test_event_id_monotonic`: verify EventId counter is monotonically increasing
   - `test_correlation_key_variants`: verify all CorrelationKey variants serialize/deserialize
2. **Regression tests:** N/A (first segment)
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo nextest run --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changed files are limited to: root `Cargo.toml`, `Cargo.lock`, `crates/prb-core/**`, `crates/prb-cli/Cargo.toml`, `crates/prb-cli/src/main.rs`, `fixtures/sample.json`. No other files.
