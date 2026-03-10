# prb-detect

Protocol detection engine for PRB. This crate identifies which protocol a network stream or datagram uses and dispatches it to the correct decoder. Detection is layered: user overrides take priority, followed by port-based mapping, magic-byte matching, and heuristic analysis. A central `DecoderRegistry` maintains per-stream decoder instances and coordinates the full detect-then-decode pipeline.

## Key types and traits

| Type / Trait | Description |
|------|-------------|
| `ProtocolDetector` | Trait — examines initial bytes + metadata to identify a protocol |
| `DetectionEngine` | Runs an ordered chain of detectors and returns the highest-confidence match |
| `DecoderRegistry` | Maps `StreamKey` → decoder instance; creates decoders on first encounter |
| `DecoderFactory` | Trait — creates stateful `ProtocolDecoder` instances for a given protocol |
| `StreamKey` | Unique identifier for a network stream (src/dst addr + transport layer) |
| `ProtocolId` | String-typed protocol identifier with well-known constants (`GRPC`, `ZMTP`, `RTPS`, …) |
| `DetectionResult` | Protocol ID + confidence score + detection method |
| `DetectionContext` | Initial bytes, ports, transport layer, and TLS status passed to detectors |

### Built-in detectors

| Detector | Strategy |
|----------|----------|
| `PortMappingDetector` | Well-known port → protocol (e.g. 50051 → gRPC) |
| `GrpcDetector` | HTTP/2 connection preface magic bytes |
| `ZmtpDetector` | ZMTP greeting signature (`0xFF…0x7F`) |
| `RtpsDetector` | `b"RTPS"` magic at byte offset 0 |
| `GuessCrateDetector` | Heuristic fallback using the `guess` crate |

## Usage

```rust
use prb_detect::{DetectionEngine, DecoderRegistry, DecoderFactory, StreamKey, TransportLayer};

// Build detection engine with built-in detectors
let engine = DetectionEngine::with_defaults();

// Detect protocol from initial bytes
let ctx = prb_detect::DetectionContext {
    initial_bytes: &payload[..256.min(payload.len())],
    src_port: 50051,
    dst_port: 49200,
    transport: TransportLayer::Tcp,
    tls_decrypted: false,
};

if let Some(result) = engine.detect(&ctx) {
    println!("Detected {} (confidence {:.0}%)", result.protocol.0, result.confidence * 100.0);
}
```

## Relationship to other crates

- **prb-core** — defines `ProtocolDecoder` and `DebugEvent` that this crate dispatches to/from
- **prb-grpc**, **prb-zmq**, **prb-dds** — supply `DecoderFactory` implementations registered in the `DecoderRegistry`
- **prb-plugin-native**, **prb-plugin-wasm** — register plugin-provided detectors and factories at runtime
- **prb-tui** — uses the registry for live and offline decoding

See the [PRB documentation](../../docs/) for the full user guide.
