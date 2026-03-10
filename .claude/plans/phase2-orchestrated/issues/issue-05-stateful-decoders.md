---
issue: 5
title: "Decoder state management across stream chunks"
severity: High
segments_affected: [2, 3]
status: open
---

# Issue 5: Decoder State Management Across Stream Chunks

## Problem

Protocol decoders are stateful:

- **GrpcDecoder**: Maintains HPACK dynamic table, HTTP/2 connection settings,
  per-stream state (headers, data accumulation, trailers)
- **ZmqDecoder**: Tracks greeting state machine (signature → version → security
  → handshake → traffic), multipart message accumulation
- **DdsDecoder**: Tracks SEDP discovery state (writer GUID → topic name mapping)

The pipeline may deliver data for the same TCP connection in multiple chunks
(multiple `ReassembledStream` events for the same 4-tuple). The decoder must
receive all chunks in order, using the same instance, to maintain state correctly.

## Current Pipeline Behavior

`TcpReassembler` emits `StreamEvent::Data(ReassembledStream)` events. Each
`ReassembledStream` contains:
- `src_ip`, `dst_ip`, `src_port`, `dst_port` (identifies the connection)
- `data: Vec<u8>` (the reassembled bytes for this segment)
- `timestamp_us` (timestamp of first packet in this segment)

**Question**: Does the reassembler emit one `ReassembledStream` per connection
(all data at once), or multiple events as data accumulates?

From reading the code: The reassembler appears to emit streams when connections
close or time out, meaning it likely delivers all data for a connection in one
chunk. However, for long-lived connections (e.g., gRPC streaming RPCs), data
may arrive in multiple segments.

## Impact

If the decoder receives partial data and is then replaced (or loses state),
it will:
- Fail to decode messages that span chunk boundaries
- Lose HPACK table state (gRPC), causing all subsequent headers to be garbled
- Lose SEDP discovery mappings (DDS), causing topic names to be unknown

## Mitigation

### M1: StreamKey-based decoder caching (Segment 2)

The `DecoderRegistry` caches active decoder instances keyed by `StreamKey`
(src:port + dst:port + transport). When a new chunk arrives for an existing
stream, the same decoder instance is retrieved and used.

```rust
fn process_stream(&mut self, key: &StreamKey, data: &[u8], ctx: &DecodeContext)
    -> Result<Vec<DebugEvent>, CoreError>
{
    let active = self.active_decoders.entry(key.clone()).or_insert_with(|| {
        let detection = self.engine.detect(/* ... */);
        let decoder = self.factories.get(&detection.protocol)
            .map(|f| f.create())
            .unwrap_or_else(|| /* raw passthrough */);
        ActiveDecoder { decoder, protocol: detection.protocol, detection }
    });

    active.decoder.decode_stream(data, ctx)
}
```

### M2: Bidirectional stream handling

A TCP connection has two directions (client→server and server→client). Some
decoders need to see both directions to decode correctly (e.g., HTTP/2 HPACK
tables are direction-specific).

**Approach**: Use canonical `StreamKey` ordering (lower IP:port first) so both
directions map to the same decoder instance. The decoder uses `DecodeContext.src_addr`
to determine direction.

```rust
impl StreamKey {
    fn canonical(src: &str, dst: &str, transport: TransportLayer) -> Self {
        if src < dst {
            Self { src_addr: src.into(), dst_addr: dst.into(), transport }
        } else {
            Self { src_addr: dst.into(), dst_addr: src.into(), transport }
        }
    }
}
```

### M3: Decoder `reset()` method

Add an optional method to `ProtocolDecoder`:

```rust
pub trait ProtocolDecoder {
    fn protocol(&self) -> TransportKind;
    fn decode_stream(&mut self, data: &[u8], ctx: &DecodeContext)
        -> Result<Vec<DebugEvent>, CoreError>;

    /// Reset internal state. Called when a stream is closed/timed out.
    /// Default implementation does nothing.
    fn reset(&mut self) {}
}
```

Called when:
- TCP FIN/RST detected (connection closed)
- Idle timeout exceeded (connection assumed dead)
- User explicitly requests cleanup

### M4: DDS cross-datagram state

DDS is special: SEDP discovery datagrams establish topic name mappings that
are used by subsequent data datagrams from different writer GUIDs. The
`DdsDecoder` maintains a `DiscoveryTracker` that persists across datagrams.

For DDS, the decoder instance should be per-domain (not per-datagram), since
discovery information is domain-wide. The registry should use a domain-scoped
key for DDS:

```rust
// For DDS, use domain_id as the stream key (not per-datagram src/dst)
let key = StreamKey {
    src_addr: format!("dds-domain-{}", domain_id),
    dst_addr: "broadcast".into(),
    transport: TransportLayer::Udp,
};
```

Alternatively, maintain a single `DdsDecoder` instance per pipeline run that
sees all RTPS traffic (simpler, avoids key complexity).

## WASM Plugin State

WASM decoder instances maintain state inside the WASM sandbox. Each
`WasmDecoderInstance` wraps a persistent Extism `Plugin` that accumulates
state across `decode_stream()` calls. The plugin's memory persists between
calls (Extism does not reset the instance between function calls).

## Acceptance Criteria

- Multi-chunk gRPC stream: decoder produces events only when complete messages
  assemble, HPACK table maintained across chunks
- Bidirectional stream: client→server and server→client chunks routed to same
  decoder instance
- Stream cleanup: decoder state freed when connection closes
- DDS discovery: topic names resolved across multiple datagrams
- WASM plugins: state persists across decode_stream() calls within same stream
