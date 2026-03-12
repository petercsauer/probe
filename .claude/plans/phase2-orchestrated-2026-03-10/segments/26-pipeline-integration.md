---
segment: 26
title: "Pipeline Integration — Wire Decoders into PcapCaptureAdapter"
crate: prb-pcap, prb-cli
status: pending
depends_on: [24, 25]
estimated_effort: "6-8 hours"
risk: 6/10
---

# Segment 3: Pipeline Integration

## Objective

Modify `PcapCaptureAdapter` to use the `DecoderRegistry` from Segment 2 so that
TCP streams and UDP datagrams are automatically detected and decoded. After this
segment, `prb ingest capture.pcap` produces `Grpc`, `Zmq`, and `DdsRtps` events
instead of `RawTcp`/`RawUdp`.

## Context

Currently, `PcapCaptureAdapter::process_tcp_stream()` creates a `RawTcp` event
with undecoded bytes. Similarly, `process_udp_datagram()` creates `RawUdp`. The
decoder crates exist but are not called.

After this segment, the pipeline becomes:

```
PCAP Reader → Normalizer → TCP Reassembly → TLS Decryption
                                                   │
                                         ┌─────────┴──────────┐
                                         ▼                    ▼
                                   DecoderRegistry      DecoderRegistry
                                   .process_stream()    .process_datagram()
                                         │                    │
                                         ▼                    ▼
                                   Decoded DebugEvents   Decoded DebugEvents
                                   (Grpc / Zmq)         (DdsRtps)
```

## Changes to `PcapCaptureAdapter`

### New field

```rust
pub struct PcapCaptureAdapter {
    capture_path: PathBuf,
    tls_keylog_path: Option<PathBuf>,
    event_queue: VecDeque<Result<DebugEvent, CoreError>>,
    stats: PipelineStats,
    initialized: bool,
    registry: DecoderRegistry,  // NEW
}
```

### Constructor changes

```rust
impl PcapCaptureAdapter {
    /// Creates a new adapter with default detection and built-in decoders.
    pub fn new(capture_path: PathBuf, tls_keylog_path: Option<PathBuf>) -> Self {
        Self {
            capture_path,
            tls_keylog_path,
            event_queue: VecDeque::new(),
            stats: PipelineStats::default(),
            initialized: false,
            registry: DecoderRegistry::with_builtins(),
        }
    }

    /// Creates a new adapter with a custom decoder registry.
    pub fn with_registry(
        capture_path: PathBuf,
        tls_keylog_path: Option<PathBuf>,
        registry: DecoderRegistry,
    ) -> Self { /* ... */ }

    /// Set a protocol override (forces all streams to use this decoder).
    pub fn set_protocol_override(&mut self, protocol: &str) {
        self.registry.set_protocol_override(ProtocolId::new(protocol));
    }
}
```

### Modified `process_tcp_stream()`

```rust
fn process_tcp_stream(
    &mut self,
    stream: ReassembledStream,
    tls_processor: &mut TlsStreamProcessor,
) {
    let decrypted = match tls_processor.process_stream(stream) {
        Ok(dec) => dec,
        Err(e) => {
            tracing::warn!("TLS processing error: {}", e);
            return;
        }
    };

    if decrypted.encrypted {
        self.stats.tls_encrypted += 1;
    } else {
        self.stats.tls_decrypted += 1;
    }

    // Build stream key for decoder routing
    let key = StreamKey {
        src_addr: format!("{}:{}", decrypted.src_ip, decrypted.src_port),
        dst_addr: format!("{}:{}", decrypted.dst_ip, decrypted.dst_port),
        transport: TransportLayer::Tcp,
    };

    // Build decode context
    let ctx = DecodeContext {
        src_addr: format!("{}:{}", decrypted.src_ip, decrypted.src_port),
        dst_addr: format!("{}:{}", decrypted.dst_ip, decrypted.dst_port),
        timestamp: Timestamp::from_nanos(decrypted.timestamp_us * 1000),
        metadata: {
            let mut m = std::collections::BTreeMap::new();
            m.insert(
                "pcap.tls_decrypted".to_string(),
                (!decrypted.encrypted).to_string(),
            );
            m.insert(
                "pcap.origin".to_string(),
                self.capture_path.display().to_string(),
            );
            m
        },
    };

    // Route through decoder registry
    match self.registry.process_stream(&key, &decrypted.data, &ctx) {
        Ok(events) if !events.is_empty() => {
            self.stats.protocol_decoded += events.len() as u64;
            for event in events {
                self.event_queue.push_back(Ok(event));
            }
        }
        Ok(_) => {
            // No events produced — emit raw fallback
            let event = self.create_raw_tcp_event(&decrypted);
            self.event_queue.push_back(Ok(event));
        }
        Err(e) => {
            // Decode failed — emit raw event with warning
            let mut event = self.create_raw_tcp_event(&decrypted);
            event.warnings.push(format!("decode failed: {}", e));
            self.event_queue.push_back(Ok(event));
        }
    }
}
```

### Modified `process_udp_datagram()`

```rust
fn process_udp_datagram(
    &mut self,
    normalized: &NormalizedPacket,
    src_port: u16,
    dst_port: u16,
) {
    let key = StreamKey {
        src_addr: format!("{}:{}", normalized.src_ip, src_port),
        dst_addr: format!("{}:{}", normalized.dst_ip, dst_port),
        transport: TransportLayer::Udp,
    };

    let ctx = DecodeContext {
        src_addr: key.src_addr.clone(),
        dst_addr: key.dst_addr.clone(),
        timestamp: Timestamp::from_nanos(normalized.timestamp_us * 1000),
        metadata: {
            let mut m = std::collections::BTreeMap::new();
            m.insert(
                "pcap.origin".to_string(),
                self.capture_path.display().to_string(),
            );
            m
        },
    };

    match self.registry.process_datagram(&key, normalized.payload, &ctx) {
        Ok(events) if !events.is_empty() => {
            self.stats.protocol_decoded += events.len() as u64;
            for event in events {
                self.event_queue.push_back(Ok(event));
            }
        }
        Ok(_) | Err(_) => {
            // Fallback: raw UDP event (existing behavior)
            let event = self.create_raw_udp_event(normalized, src_port, dst_port);
            self.event_queue.push_back(Ok(event));
        }
    }
}
```

### Extract helper methods

Refactor `create_debug_event_from_stream()` into two focused helpers:

```rust
fn create_raw_tcp_event(&self, stream: &DecryptedStream) -> DebugEvent { /* ... */ }
fn create_raw_udp_event(&self, normalized: &NormalizedPacket, src: u16, dst: u16) -> DebugEvent { /* ... */ }
```

### Updated `PipelineStats`

```rust
pub struct PipelineStats {
    pub packets_read: u64,
    pub packets_failed: u64,
    pub tcp_streams: u64,
    pub udp_datagrams: u64,
    pub tls_decrypted: u64,
    pub tls_encrypted: u64,
    pub protocol_decoded: u64,    // NEW: events decoded by protocol decoders
    pub protocol_fallback: u64,   // NEW: streams that fell back to raw
}
```

## Changes to `prb-pcap/Cargo.toml`

```toml
[dependencies]
prb-core = { path = "../prb-core" }
prb-detect = { path = "../prb-detect" }  # NEW — replaces direct decoder deps
# ... (keep existing deps)
```

**Remove**: `prb-pcap` no longer needs to depend on `prb-grpc`, `prb-zmq`,
`prb-dds` directly. Those dependencies move to `prb-detect`.

## Changes to `prb-cli`

### `--protocol` flag

Add a `--protocol` CLI option to the `ingest` command:

```rust
#[derive(Parser)]
struct IngestArgs {
    /// Path to capture file (PCAP/pcapng/JSON)
    input: PathBuf,

    /// TLS keylog file for decryption
    #[arg(long)]
    tls_keylog: Option<PathBuf>,

    /// Force protocol detection to a specific protocol.
    /// Bypasses auto-detection for all streams.
    #[arg(long, value_parser = ["grpc", "zmtp", "rtps"])]
    protocol: Option<String>,

    /// Custom port-to-protocol mappings (e.g., "9090=grpc,6789=zmtp").
    #[arg(long)]
    port_map: Option<String>,
}
```

### Wire `--protocol` to adapter

```rust
fn run_ingest(args: IngestArgs) -> Result<()> {
    let mut adapter = PcapCaptureAdapter::new(args.input, args.tls_keylog);

    if let Some(protocol) = args.protocol {
        adapter.set_protocol_override(&protocol);
    }

    // ... rest of ingest
}
```

## Test Fixtures

### Create `fixtures/mixed_protocol.pcap`

A test pcapng file containing:
1. A gRPC unary call (HTTP/2 preface + HEADERS + DATA)
2. A ZMQ PUB/SUB exchange (ZMTP greeting + READY + message)
3. A DDS RTPS discovery + data exchange

This can be synthesized from the existing test helpers in each decoder crate,
or captured from a live setup.

**Alternative**: Create `fixtures/mixed_protocol_synthetic.rs` — a test helper
that builds raw packet bytes for each protocol and wraps them in pcap format
using the `pcap-file` crate for writing.

### Integration tests

```rust
// prb-pcap/tests/protocol_detection_tests.rs

#[test]
fn test_grpc_stream_auto_detected() {
    // Feed HTTP/2 preface + gRPC frames through pipeline
    // Assert events have TransportKind::Grpc
    // Assert grpc.method metadata present
}

#[test]
fn test_zmtp_stream_auto_detected() {
    // Feed ZMTP greeting + messages through pipeline
    // Assert events have TransportKind::Zmq
    // Assert zmq.socket_type metadata present
}

#[test]
fn test_rtps_datagram_auto_detected() {
    // Feed RTPS-headed UDP datagrams through pipeline
    // Assert events have TransportKind::DdsRtps
    // Assert dds.domain_id metadata present
}

#[test]
fn test_unknown_protocol_falls_back() {
    // Feed random TCP data through pipeline
    // Assert events have TransportKind::RawTcp
    // Assert no decoder error
}

#[test]
fn test_protocol_override() {
    // Set --protocol grpc, feed non-HTTP/2 data
    // Assert decoder attempted gRPC decode (may fail → raw with warning)
}

#[test]
fn test_mixed_protocols_in_same_capture() {
    // Feed multiple streams with different protocols
    // Assert each correctly detected and decoded
}
```

## Tasks

### T3.1: Add `prb-detect` dependency to `prb-pcap`
- Update `prb-pcap/Cargo.toml`
- Remove direct decoder crate dependencies from `prb-pcap`
- Verify workspace compiles

### T3.2: Modify `PcapCaptureAdapter` to hold `DecoderRegistry`
- Add `registry` field
- Update constructor
- Add `with_registry()` factory method
- Add `set_protocol_override()` method

### T3.3: Rewrite `process_tcp_stream()` to use registry
- Build `StreamKey` from stream metadata
- Build `DecodeContext` from stream metadata
- Call `registry.process_stream()`
- Handle decoded events, empty results, and errors
- Preserve raw fallback behavior

### T3.4: Rewrite `process_udp_datagram()` to use registry
- Build `StreamKey` from datagram metadata
- Call `registry.process_datagram()`
- Handle decoded events with fallback
- Preserve raw fallback behavior

### T3.5: Extract raw event helper methods
- `create_raw_tcp_event()`
- `create_raw_udp_event()`
- Update `PipelineStats` with `protocol_decoded` and `protocol_fallback`

### T3.6: Update CLI with `--protocol` and `--port-map` flags
- Add flags to `IngestArgs`
- Wire to `PcapCaptureAdapter`
- Parse `--port-map` format

### T3.7: Create test fixtures
- Synthetic pcap builder helper (or real captures)
- Test helper to create HTTP/2/ZMTP/RTPS byte streams wrapped in pcap

### T3.8: Write integration tests
- All 6 test cases described above
- Verify no regression in existing pipeline tests
- Run full `cargo test --workspace`

### T3.9: Update pipeline stats logging
- Log detection results per stream
- Log decoder success/failure rates
- Add `tracing::info!` for protocol detection results

## Risk: Decoder State and Stream Chunking

**Problem**: The current pipeline calls `process_tcp_stream()` once per
`ReassembledStream`. But a single TCP connection may produce multiple
`ReassembledStream` events (e.g., if the connection is long-lived and data
arrives in chunks). The decoder needs all chunks of the same connection.

**Mitigation**: The `StreamKey`-based caching in `DecoderRegistry` handles this.
The same decoder instance is reused for all chunks of the same connection. The
decoder's `decode_stream()` is called incrementally, accumulating state.

**Verification**: Test with multi-chunk gRPC streams (request body split across
TCP segments) — the decoder should produce events only when complete messages
are assembled.

See also: `issues/issue-05-stateful-decoders.md`

## Verification

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

After this segment:
- `prb ingest fixtures/grpc_sample.json` continues to work (no regression)
- `prb ingest capture_with_grpc.pcap` produces `Grpc` events with metadata
- `prb ingest capture_with_mixed.pcap` produces correct event types per stream
- Pipeline stats show protocol_decoded > 0
