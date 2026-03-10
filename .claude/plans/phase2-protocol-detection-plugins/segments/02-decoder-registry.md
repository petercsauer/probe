---
segment: 2
title: "DecoderRegistry + Dispatch Layer"
crate: prb-detect
status: pending
depends_on: [1]
estimated_effort: "4-6 hours"
risk: 4/10
---

# Segment 2: DecoderRegistry + Dispatch Layer

## Objective

Build the `DecoderRegistry` that owns protocol detectors and decoders, and the
dispatch layer that routes detected streams to the correct decoder. This is the
central coordination point that the pipeline will call.

## Context

Segment 1 gave us the `DetectionEngine` that can identify a protocol from bytes.
Now we need to:
1. Map detected `ProtocolId` → `ProtocolDecoder` instances
2. Handle the full detect → decode workflow in one call
3. Support registering built-in decoders and (later) plugin decoders identically
4. Manage decoder state (decoders are stateful — they track per-stream state)

## Key Design: Stream-Keyed Decoder Instances

Protocol decoders (especially `GrpcDecoder` and `ZmqDecoder`) are **stateful** —
they accumulate partial data across multiple `decode_stream` calls for the same
TCP connection. This means we need one decoder instance **per stream**, not one
per protocol.

```
DecoderRegistry
├── detection_engine: DetectionEngine
├── decoder_factories: HashMap<ProtocolId, Box<dyn DecoderFactory>>
└── active_decoders: HashMap<StreamKey, Box<dyn ProtocolDecoder>>
```

### `StreamKey` — Identifies a unique stream

```rust
/// Unique identifier for a network stream (TCP connection or UDP flow).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamKey {
    pub src_addr: String,  // "ip:port"
    pub dst_addr: String,  // "ip:port"
    pub transport: TransportLayer,
}
```

### `DecoderFactory` trait

```rust
/// Creates new decoder instances for a given protocol.
///
/// This is needed because decoders are stateful (per-stream state).
/// The factory creates a fresh decoder for each new stream.
pub trait DecoderFactory: Send + Sync {
    /// The protocol this factory creates decoders for.
    fn protocol_id(&self) -> &ProtocolId;

    /// Create a new decoder instance.
    fn create(&self) -> Box<dyn ProtocolDecoder>;

    /// Human-readable name of the decoder.
    fn name(&self) -> &str;

    /// Description of the decoder.
    fn description(&self) -> &str;

    /// Version of the decoder.
    fn version(&self) -> &str;
}
```

### Built-in Decoder Factories

```rust
pub struct GrpcDecoderFactory;
impl DecoderFactory for GrpcDecoderFactory {
    fn protocol_id(&self) -> &ProtocolId { &ProtocolId::new(ProtocolId::GRPC) }
    fn create(&self) -> Box<dyn ProtocolDecoder> { Box::new(GrpcDecoder::new()) }
    fn name(&self) -> &str { "gRPC/HTTP2" }
    fn description(&self) -> &str { "Decodes gRPC over HTTP/2 with HPACK and LPM" }
    fn version(&self) -> &str { env!("CARGO_PKG_VERSION") }
}

pub struct ZmqDecoderFactory;
// Same pattern...

pub struct DdsDecoderFactory;
// Same pattern...
```

## DecoderRegistry

```rust
/// Central registry managing protocol detection and decoding.
///
/// Owns the detection engine, decoder factories, and active decoder
/// instances. The pipeline calls `process_stream()` or `process_datagram()`
/// which handles detection + decoding in one step.
pub struct DecoderRegistry {
    engine: DetectionEngine,
    factories: HashMap<ProtocolId, Box<dyn DecoderFactory>>,
    active_decoders: HashMap<StreamKey, ActiveDecoder>,
    user_override: Option<ProtocolId>,
}

struct ActiveDecoder {
    decoder: Box<dyn ProtocolDecoder>,
    protocol: ProtocolId,
    detection: DetectionResult,
}
```

### Core Methods

```rust
impl DecoderRegistry {
    /// Create a registry with all built-in detectors and decoders.
    pub fn with_builtins() -> Self { /* ... */ }

    /// Register a custom decoder factory.
    pub fn register_decoder(&mut self, factory: Box<dyn DecoderFactory>) { /* ... */ }

    /// Register a custom protocol detector.
    pub fn register_detector(&mut self, detector: Box<dyn ProtocolDetector>) { /* ... */ }

    /// Set a user protocol override (applies to all streams).
    pub fn set_protocol_override(&mut self, protocol: ProtocolId) { /* ... */ }

    /// Process a reassembled TCP stream.
    ///
    /// Detects the protocol (if not already known for this stream), then
    /// routes to the appropriate decoder. Returns decoded DebugEvents.
    pub fn process_stream(
        &mut self,
        key: &StreamKey,
        data: &[u8],
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError> { /* ... */ }

    /// Process a UDP datagram.
    ///
    /// Detects the protocol and decodes. UDP decoders are stateless per
    /// datagram (though DDS tracks discovery state across datagrams).
    pub fn process_datagram(
        &mut self,
        key: &StreamKey,
        data: &[u8],
        ctx: &DecodeContext,
    ) -> Result<Vec<DebugEvent>, CoreError> { /* ... */ }

    /// Get info about all registered decoders.
    pub fn list_decoders(&self) -> Vec<DecoderInfo> { /* ... */ }

    /// Clean up decoder state for closed/timed-out streams.
    pub fn cleanup_stream(&mut self, key: &StreamKey) { /* ... */ }
}
```

### Dispatch Logic

The `process_stream` method implements this flow:

```
process_stream(key, data, ctx)
│
├── Is user_override set?
│   ├── Yes → use override protocol
│   └── No → check active_decoders for key
│       ├── Found → reuse existing decoder
│       └── Not found → run detection_engine.detect()
│           ├── Known protocol → create decoder from factory
│           └── Unknown → return raw event with warning
│
├── Get/create decoder for stream
│
├── Call decoder.decode_stream(data, ctx)
│   ├── Ok(events) → return events with transport set to detected protocol
│   └── Err(e) → return raw event with decode error as warning
│
└── If decoder returned empty and data.len() > 0:
    → return raw event with "incomplete decode" warning
```

### Protocol ↔ TransportKind Mapping

The `ProtocolId` from detection maps to `TransportKind` for events:

```rust
fn protocol_to_transport(id: &ProtocolId) -> TransportKind {
    match id.as_str() {
        ProtocolId::GRPC | ProtocolId::HTTP2 => TransportKind::Grpc,
        ProtocolId::ZMTP => TransportKind::Zmq,
        ProtocolId::RTPS => TransportKind::DdsRtps,
        _ => TransportKind::RawTcp, // or RawUdp based on transport
    }
}
```

### `DecoderInfo` — For CLI listing

```rust
#[derive(Debug, Clone)]
pub struct DecoderInfo {
    pub protocol_id: ProtocolId,
    pub name: String,
    pub description: String,
    pub version: String,
    pub source: DecoderSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecoderSource {
    BuiltIn,
    NativePlugin { path: PathBuf },
    WasmPlugin { path: PathBuf },
}
```

## Tasks

### T2.1: Define `StreamKey`, `DecoderFactory`, `DecoderInfo`, `DecoderSource`
- Add types to `prb-detect/src/registry.rs`
- `StreamKey` implements `Hash + Eq` for use as HashMap key

### T2.2: Implement `DecoderRegistry` core structure
- Constructor `with_builtins()` registers all built-in detector/decoder pairs
- `register_decoder()` and `register_detector()` for extensibility
- `list_decoders()` returns metadata about all registered decoders
- `set_protocol_override()` for `--protocol` CLI flag

### T2.3: Implement `process_stream()` dispatch
- Detection → factory lookup → decoder creation → decode
- Cache active decoders by `StreamKey`
- Handle detection failure → raw event fallback
- Handle decode failure → raw event with warning
- Test: HTTP/2 preface bytes → gRPC decoder invoked
- Test: ZMTP greeting → ZMQ decoder invoked
- Test: Unknown bytes → raw fallback

### T2.4: Implement `process_datagram()` dispatch
- Detection → factory lookup → decode
- DDS decoder maintains cross-datagram state (SEDP discovery)
- Test: RTPS magic bytes → DDS decoder invoked
- Test: Non-RTPS UDP → raw fallback

### T2.5: Implement built-in `DecoderFactory` for gRPC, ZMQ, DDS
- `GrpcDecoderFactory`, `ZmqDecoderFactory`, `DdsDecoderFactory`
- Each wraps the corresponding crate's decoder
- Test: factory creates working decoder instance

### T2.6: Implement `cleanup_stream()`
- Remove active decoder for a closed stream
- Free associated state
- Test: cleanup removes decoder, new data triggers fresh detection

### T2.7: Implement user override
- `set_protocol_override()` bypasses detection entirely
- Override applies to ALL streams (useful for `--protocol grpc`)
- Test: override set → all streams decoded as specified protocol

### T2.8: Integration tests
- Create test with mixed protocol data (HTTP/2 + ZMTP + RTPS)
- Verify each stream routed to correct decoder
- Verify decoded events have correct `TransportKind`
- Verify unknown streams produce `RawTcp`/`RawUdp` with warnings

## Dependencies

`prb-detect` must now depend on the decoder crates:

```toml
[dependencies]
prb-core = { path = "../prb-core" }
prb-grpc = { path = "../prb-grpc" }
prb-zmq = { path = "../prb-zmq" }
prb-dds = { path = "../prb-dds" }
guess = { version = "0.2", features = ["full"] }
tracing.workspace = true
```

**Note**: This creates a one-way dependency from `prb-detect` → decoder crates.
The decoder crates remain independent. The pipeline (`prb-pcap`) will depend on
`prb-detect` instead of depending on each decoder crate individually.

## Verification

```bash
cargo test -p prb-detect
cargo clippy -p prb-detect -- -D warnings
```

Registry correctly routes detection results to decoders and produces typed
`DebugEvent`s instead of raw events.
