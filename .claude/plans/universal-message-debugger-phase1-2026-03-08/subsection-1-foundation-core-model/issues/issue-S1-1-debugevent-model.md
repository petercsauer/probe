---
id: "S1-1"
title: "DebugEvent Canonical Model Under-specified"
risk: 4/10
addressed_by_segments: [1]
---
# Issue S1-1: DebugEvent Canonical Model Under-specified

**Core Problem:**
The parent plan states "The DebugEvent type that every later subsection produces or consumes" but defines zero fields. This is the single most critical type in the project -- every one of the 14 planned crates and all 5 subsections either produces, transforms, or consumes DebugEvents. Getting the field set wrong means refactoring the entire project.

**Root Cause:**
The parent plan was scoped as a decomposition plan, not an implementation plan. It deferred field-level design to the deep-plan phase.

**Proposed Fix:**
Define the DebugEvent struct and its supporting types with fields informed by three reference models (MCAP Message, Wireshark packet, CloudEvents spec) and the needs of all 5 subsections:

```rust
pub struct DebugEvent {
    pub id: EventId,
    pub timestamp: Timestamp,
    pub source: EventSource,
    pub transport: TransportKind,
    pub direction: Direction,
    pub payload: Payload,
    pub metadata: BTreeMap<String, String>,
    pub correlation_keys: Vec<CorrelationKey>,
    pub sequence: Option<u64>,
    pub warnings: Vec<String>,
}
```

Supporting types:

- `EventId`: newtype over `uuid::Uuid` (or a monotonic u64 counter for perf)
- `Timestamp`: wrapper holding nanoseconds since Unix epoch (maps to MCAP `log_time`)
- `EventSource`: struct with `adapter: String`, `origin: String`, optional `NetworkAddr`
- `NetworkAddr`: `src_addr`, `src_port`, `dst_addr`, `dst_port`
- `TransportKind`: enum `{ Grpc, Zmq, DdsRtps, RawTcp, RawUdp, JsonFixture }`
- `Direction`: enum `{ Inbound, Outbound, Unknown }`
- `Payload`: enum `{ Raw(Bytes), Decoded { raw: Bytes, fields: serde_json::Value, schema_name: Option<String> } }`
- `CorrelationKey`: enum `{ StreamId(u32), Topic(String), ConnectionId(String), Custom(String, String) }`

The `warnings` field carries parse warnings per the project's no-ignore-failure rule -- malformed data surfaces as warnings, not silent drops.

The `metadata` BTreeMap carries protocol-specific key-value pairs (gRPC method name, HTTP/2 stream ID, ZMQ topic, DDS domain). This avoids making DebugEvent aware of protocol-specific types while still carrying the data the correlation engine needs.

**Existing Solutions Evaluated:**
- MCAP `Message` struct (mcap v0.24.0): has `channel`, `sequence`, `log_time`, `publish_time`, `data`. Our DebugEvent is richer -- it carries decoded fields, correlation keys, and metadata that MCAP stores in Channel metadata. DebugEvent serializes into MCAP Message data, with transport/source info mapped to MCAP Channel topic and metadata.
- CloudEvents spec (CNCF v1.0.2): defines `id`, `source`, `type`, `time`, `data`, `datacontenttype`, `subject`, `extensions`. Our model adopts the same pattern (id, source, timestamp, typed payload, extensible metadata) but is domain-specific to message debugging.
- Wireshark packet model: `timestamp`, `source`, `destination`, `protocol`, `info`, `data`. Our model expands this with directional info, correlation keys, and decoded payload.

**Alternatives Considered:**
- Use an untyped `serde_json::Value` as the entire event model. Rejected: loses compile-time guarantees, makes trait signatures vague, and forces every consumer to do runtime key lookups.
- Use protobuf `DynamicMessage` (via prost-reflect) as the event model. Rejected: creates a circular dependency (the tool decodes protobuf, storing events as protobuf means decoding its own storage) and is less ergonomic than native Rust structs.

**Pre-Mortem -- What Could Go Wrong:**
- The `metadata: BTreeMap<String, String>` is too loose -- protocol decoders may use inconsistent key names, breaking the correlation engine. Mitigation: define well-known metadata key constants in prb-core (e.g., `METADATA_KEY_GRPC_METHOD`, `METADATA_KEY_H2_STREAM_ID`).
- `Payload::Decoded.fields` as `serde_json::Value` may be too heavyweight for high-throughput scenarios (100k+ events/sec). Mitigation: benchmark in Subsection 5 and consider a more compact representation if needed.
- `Bytes` from the bytes crate adds a dependency but is justified for zero-copy slicing of large capture payloads.
- Adding fields later is backward-compatible for serde (new fields deserialize as defaults), but removing or renaming fields is breaking.

**Risk Factor:** 4/10

**Evidence for Optimality:**
- External evidence: CloudEvents v1.0.2 (CNCF) uses the same pattern of required context attributes + extensible metadata for event systems, validating the struct-plus-metadata design.
- External evidence: MCAP's Channel metadata (BTreeMap<String, String>) uses the same pattern for per-channel extensible metadata, confirming that key-value pairs are the right abstraction for protocol-specific data.
- Project conventions: The `no-ignore-failure` rule (.cursor/rules/no-ignore-failure.mdc) requires surfacing errors, which motivates the `warnings: Vec<String>` field.

**Blast Radius:**
- Direct changes: `crates/prb-core/src/event.rs` (new file)
- Potential ripple: every crate in the workspace imports DebugEvent; changes to its fields require coordinated updates
