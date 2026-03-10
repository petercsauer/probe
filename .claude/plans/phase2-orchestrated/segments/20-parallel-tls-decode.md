---
segment: 20
title: "Parallel TLS Decryption + Protocol Decode"
depends_on: [19]
risk: 9
complexity: Medium
cycle_budget: 3
status: pending
commit_message: "refactor(prb-pcap): make TLS decrypt thread-safe, add protocol auto-detection in shard context"
---

# Subsection 4: Parallel TLS Decryption + Protocol Decode

## Purpose

After TCP reassembly produces `ReassembledStream`s within each shard, TLS
decryption and protocol decoding are applied per-stream. Both stages are
stateless per-stream and can run with `rayon::par_iter` within each shard
for an additional parallelism level (nested parallelism).

However, the primary parallelism is already at the shard level (S3). This
subsection focuses on making decryption and decode correct and efficient
within the shard context, with optional intra-shard parallelism for captures
dominated by a small number of heavy flows.

---

## Segment S4.1: TLS Decryption in Shard Context

### Current state

`TlsStreamProcessor::process_stream(&mut self, stream)` takes `&mut self`
only because it *could* mutate the keylog. In practice, `process_stream` only
calls `self.keylog.lookup()` which is `&self`. The `TlsSession`, `TlsDecryptor`,
and derived keys are all created fresh per stream.

### Changes

After S1.3's refactoring to `Arc<TlsKeyLog>`, the processor is effectively
stateless per call. Make this explicit:

```rust
// crates/prb-pcap/src/tls/mod.rs

impl TlsStreamProcessor {
    /// Decrypts a single stream. Thread-safe — only reads from shared keylog.
    pub fn decrypt_stream(&self, stream: ReassembledStream) -> Result<DecryptedStream, PcapError> {
        let session_result = TlsSession::from_stream(&stream.data);

        let (data, encrypted) = match session_result {
            Ok(session) => {
                if let Some(key_materials) = self.keylog.lookup(&session.client_random) {
                    let decryptor = TlsDecryptor::new(&session, key_materials)?;
                    match decryptor.decrypt_stream(&stream.data, stream.direction) {
                        Ok(decrypted) => (decrypted, false),
                        Err(_) => (stream.data, true),
                    }
                } else {
                    (stream.data, true)
                }
            }
            Err(_) => (stream.data, true),
        };

        Ok(DecryptedStream {
            src_ip: stream.src_ip,
            src_port: stream.src_port,
            dst_ip: stream.dst_ip,
            dst_port: stream.dst_port,
            direction: stream.direction,
            data,
            encrypted,
            is_complete: stream.is_complete,
            timestamp_us: stream.timestamp_us,
        })
    }
}
```

The method signature is `&self` — enabling `Arc<TlsStreamProcessor>` sharing
across rayon threads if needed.

### Optional: Intra-shard parallel TLS

For captures where one flow dominates (elephant flow producing many streams),
TLS decryption of multiple streams within a shard can be parallelized:

```rust
fn decrypt_streams_parallel(
    processor: &TlsStreamProcessor,
    streams: Vec<ReassembledStream>,
) -> Vec<DecryptedStream> {
    if streams.len() > 16 {
        // Worth parallelizing
        streams
            .into_par_iter()
            .map(|s| processor.decrypt_stream(s).unwrap_or_else(|e| {
                tracing::warn!("TLS decrypt failed: {}", e);
                DecryptedStream::pass_through(s)
            }))
            .collect()
    } else {
        // Sequential for small batches
        streams
            .into_iter()
            .filter_map(|s| processor.decrypt_stream(s).ok())
            .collect()
    }
}
```

rayon handles nested parallelism correctly via work-stealing — inner
`par_iter` tasks join the same thread pool without deadlock.

---

## Segment S4.2: Protocol Decode in Shard Context

### Current state

Protocol decoders (`GrpcDecoder`, `ZmqDecoder`, `DdsDecoder`) implement
`ProtocolDecoder::decode_stream(&mut self, data, ctx)`. They are stateful
per-connection:

- `GrpcDecoder`: H2Codec (frame state), LpmParser (message boundary state)
- `ZmqDecoder`: ZmtpParser (greeting/handshake state), socket_metadata
- `DdsDecoder`: DiscoveryTracker (GUID→topic mapping)

Since each `ReassembledStream` represents a complete connection (or direction),
a fresh decoder instance per stream is correct:

```rust
fn decode_stream(
    stream: &DecryptedStream,
    capture_path: &Path,
) -> Vec<DebugEvent> {
    if stream.encrypted {
        return vec![create_raw_event(stream, capture_path)];
    }

    let ctx = DecodeContext {
        src_addr: format!("{}:{}", stream.src_ip, stream.src_port),
        dst_addr: format!("{}:{}", stream.dst_ip, stream.dst_port),
        metadata: Default::default(),
        timestamp: Some(Timestamp::from_nanos(stream.timestamp_us * 1000)),
    };

    // Try protocol detection by port heuristics or magic bytes
    let protocol = detect_protocol(stream);

    match protocol {
        Some(DetectedProtocol::Grpc) => {
            let mut decoder = GrpcDecoder::new();
            decoder.decode_stream(&stream.data, &ctx).unwrap_or_default()
        }
        Some(DetectedProtocol::Zmtp) => {
            let mut decoder = ZmqDecoder::new();
            decoder.decode_stream(&stream.data, &ctx).unwrap_or_default()
        }
        Some(DetectedProtocol::Rtps) => {
            let mut decoder = DdsDecoder::new();
            decoder.decode_stream(&stream.data, &ctx).unwrap_or_default()
        }
        None => {
            vec![create_raw_event(stream, capture_path)]
        }
    }
}
```

### Protocol detection

Simple heuristic-based detection using initial bytes and port numbers:

```rust
enum DetectedProtocol {
    Grpc,
    Zmtp,
    Rtps,
}

fn detect_protocol(stream: &DecryptedStream) -> Option<DetectedProtocol> {
    let data = &stream.data;

    // HTTP/2 connection preface: "PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n"
    if data.len() >= 24 && &data[..24] == b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n" {
        return Some(DetectedProtocol::Grpc);
    }

    // HTTP/2 frames without preface (server side): check for frame header
    // with known frame types (0x00-0x09)
    if data.len() >= 9 {
        let frame_type = data[3];
        let frame_len = u32::from_be_bytes([0, data[0], data[1], data[2]]);
        if frame_type <= 0x09 && frame_len < 16_777_216 {
            return Some(DetectedProtocol::Grpc);
        }
    }

    // ZMTP greeting: starts with 0xFF, 8 bytes padding, 0x7F
    if data.len() >= 10 && data[0] == 0xFF && data[9] == 0x7F {
        return Some(DetectedProtocol::Zmtp);
    }

    // RTPS magic bytes (for UDP, but included for completeness)
    if data.len() >= 4 && &data[..4] == b"RTPS" {
        return Some(DetectedProtocol::Rtps);
    }

    // Port-based fallback
    let grpc_ports = [50051, 443, 8443, 9090];
    if grpc_ports.contains(&stream.dst_port) || grpc_ports.contains(&stream.src_port) {
        return Some(DetectedProtocol::Grpc);
    }

    None
}
```

This is a simplified version of the full protocol auto-detection planned in
competitive analysis #7. It covers the three protocols Probe currently supports
and provides the integration point for the full detection system later.

### DDS special case

DDS/RTPS travels over UDP multicast. It doesn't go through TCP reassembly or
TLS. In the shard processing, UDP packets with RTPS magic bytes are decoded
directly:

```rust
// In shard processing loop for UDP packets
if packet.payload.len() >= 4 && &packet.payload[..4] == b"RTPS" {
    let mut decoder = DdsDecoder::new();
    let ctx = DecodeContext { /* ... */ };
    match decoder.decode_stream(&packet.payload, &ctx) {
        Ok(decoded_events) => events.extend(decoded_events),
        Err(e) => {
            tracing::warn!("DDS decode failed: {}", e);
            events.push(create_raw_udp_event(packet));
        }
    }
} else {
    events.push(create_raw_udp_event(packet));
}
```

---

## Thread Safety Summary

| Component | `Send` | `Sync` | Sharing strategy |
|-----------|--------|--------|-----------------|
| `TlsKeyLog` | Yes | Yes | `Arc<TlsKeyLog>`, read-only lookups |
| `TlsStreamProcessor` | Yes | Yes | One per shard, or shared via `Arc` |
| `TlsSession` | Yes | No | Created fresh per stream, not shared |
| `TlsDecryptor` | Yes | No | Created fresh per stream, not shared |
| `GrpcDecoder` | Yes | No | Fresh instance per stream |
| `ZmqDecoder` | Yes | No | Fresh instance per stream |
| `DdsDecoder` | Yes | No | Fresh instance per stream |

All decoder state is contained within per-stream decoder instances. No
cross-stream sharing needed.

---

## Files Changed

| File | Change |
|------|--------|
| `crates/prb-pcap/src/tls/mod.rs` | Change `process_stream` to `&self`, add `decrypt_stream` |
| `crates/prb-pcap/src/parallel/shard.rs` | Add protocol detection + decode dispatch |
| `crates/prb-pcap/src/parallel/detect.rs` | New: `detect_protocol`, `DetectedProtocol` |

---

## Tests

- `test_tls_decrypt_is_send_sync` — Static assert: `TlsStreamProcessor: Send + Sync`
- `test_tls_decrypt_parallel_same_keylog` — Multiple streams decrypted in
  parallel sharing same Arc<TlsKeyLog>
- `test_detect_http2_preface` — HTTP/2 preface bytes → Grpc
- `test_detect_http2_frames` — Server-side H2 frames → Grpc
- `test_detect_zmtp_greeting` — ZMTP greeting → Zmtp
- `test_detect_rtps_magic` — RTPS header → Rtps
- `test_detect_port_fallback` — Port 50051 without magic → Grpc
- `test_detect_unknown` — Random bytes → None
- `test_decode_grpc_in_shard` — gRPC stream decoded correctly in shard context
- `test_decode_zmtp_in_shard` — ZMTP stream decoded correctly in shard context
