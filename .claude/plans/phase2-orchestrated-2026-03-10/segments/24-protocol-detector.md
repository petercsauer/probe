---
segment: 24
title: "ProtocolDetector Trait + Built-in Detectors"
crate: prb-detect
status: pending
depends_on: []
estimated_effort: "4-6 hours"
risk: 3/10
---

# Segment 1: ProtocolDetector Trait + Built-in Detectors

## Objective

Define the `ProtocolDetector` trait and implement built-in detectors for the three
supported protocols (gRPC/HTTP2, ZMTP, DDS/RTPS) plus a `guess`-backed generic
detector. This segment creates the `prb-detect` crate.

## Context

The pipeline currently emits `RawTcp`/`RawUdp` because there is no mechanism to
identify the application-layer protocol. Detection must happen *after* TCP
reassembly and TLS decryption but *before* protocol decoding.

Detection inputs:
- **TCP streams**: Reassembled, optionally TLS-decrypted byte stream
- **UDP datagrams**: Individual UDP payloads
- **Metadata**: src/dst IP:port, TLS status, timestamp

## New Crate: `prb-detect`

### Cargo.toml

```toml
[package]
name = "prb-detect"
version.workspace = true
edition.workspace = true

[dependencies]
prb-core = { path = "../prb-core" }
guess = { version = "0.2", features = ["full"] }
tracing.workspace = true
```

## Types and Traits

### `ProtocolId` — Identifier for detected protocols

```rust
/// Unique identifier for a protocol.
///
/// Built-in protocols use well-known string IDs. Plugin protocols use
/// their registered name (e.g., "thrift", "capnproto").
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProtocolId(pub String);

impl ProtocolId {
    pub const GRPC: &str = "grpc";
    pub const ZMTP: &str = "zmtp";
    pub const RTPS: &str = "rtps";
    pub const HTTP2: &str = "http2";
    pub const HTTP1: &str = "http1";
    pub const TLS: &str = "tls";
    pub const UNKNOWN: &str = "unknown";
}
```

### `DetectionResult` — Output of detection

```rust
/// Result of protocol detection.
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Detected protocol identifier.
    pub protocol: ProtocolId,
    /// Confidence level (0.0 = guess, 1.0 = certain).
    pub confidence: f32,
    /// How the protocol was detected.
    pub method: DetectionMethod,
    /// Optional protocol version (e.g., "3.1" for ZMTP).
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionMethod {
    /// User explicitly specified the protocol.
    UserOverride,
    /// Matched a known port mapping.
    PortMapping,
    /// Matched magic bytes in the payload.
    MagicBytes,
    /// Heuristic analysis of payload content.
    Heuristic,
    /// No detection succeeded; using fallback.
    Fallback,
}
```

### `DetectionContext` — Input to detection

```rust
/// Context available during protocol detection.
pub struct DetectionContext<'a> {
    /// First bytes of the stream/datagram (up to 256 bytes).
    pub initial_bytes: &'a [u8],
    /// Source port.
    pub src_port: u16,
    /// Destination port.
    pub dst_port: u16,
    /// Transport layer (tcp or udp).
    pub transport: TransportLayer,
    /// Whether the stream was TLS-decrypted.
    pub tls_decrypted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportLayer {
    Tcp,
    Udp,
}
```

### `ProtocolDetector` trait

```rust
/// Detects whether a byte stream or datagram matches a specific protocol.
///
/// Detectors are ordered by priority. The first detector that returns a
/// result with confidence >= threshold wins.
pub trait ProtocolDetector: Send + Sync {
    /// Human-readable name for logging (e.g., "grpc-magic-bytes").
    fn name(&self) -> &str;

    /// Which transport layer this detector applies to.
    fn transport(&self) -> TransportLayer;

    /// Attempt to detect the protocol from the initial bytes and metadata.
    ///
    /// Returns `Some(result)` if the protocol was identified, `None` if
    /// this detector cannot determine the protocol.
    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult>;
}
```

## Built-in Detector Implementations

### 1. `PortMappingDetector`

Static port → protocol mapping. Lowest latency, used first.

```rust
pub struct PortMappingDetector {
    tcp_mappings: HashMap<u16, ProtocolId>,
    udp_mappings: HashMap<u16, ProtocolId>,
}
```

**Default mappings:**
- TCP 50051 → gRPC
- TCP 80, 8080, 443, 8443 → HTTP/2 (tentative, needs magic-byte confirmation)
- UDP 7400-7500 → RTPS
- TCP 5555, 5556 → ZMTP (common ZMQ ports)

Users can extend via `--port-map "9090=grpc,6789=zmtp"` CLI argument.

Confidence: 0.5 (port alone is not definitive, but prioritizes the right detector).

### 2. `GuessCrateDetector`

Wraps the `guess` crate for zero-copy detection of common protocols.

```rust
pub struct GuessCrateDetector {
    tcp_detector: guess::ProtocolDetector<guess::TcpState>,
    udp_detector: guess::ProtocolDetector<guess::UdpState>,
}
```

Maps `guess::ProtocolInfo` → `DetectionResult`:
- `guess` HTTP/2 → `ProtocolId::HTTP2` (confidence 0.9)
- `guess` TLS → `ProtocolId::TLS` (confidence 0.9)

Confidence: 0.85-0.95 (magic-byte match is strong but not infallible).

**Limitation**: `guess` does not detect ZMTP or RTPS — those need custom detectors.

### 3. `GrpcDetector` (custom magic-byte)

Detects HTTP/2 connection preface: `PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n` (24 bytes).
Also detects HTTP/2 frames without preface (mid-stream: frame header with valid
type byte and reasonable length).

```rust
pub struct GrpcDetector;

impl ProtocolDetector for GrpcDetector {
    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult> {
        if ctx.transport != TransportLayer::Tcp { return None; }

        // Check for HTTP/2 connection preface
        const H2_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
        if ctx.initial_bytes.starts_with(H2_PREFACE) {
            return Some(DetectionResult {
                protocol: ProtocolId::new(ProtocolId::GRPC),
                confidence: 0.95,
                method: DetectionMethod::MagicBytes,
                version: Some("h2".into()),
            });
        }

        // Heuristic: HTTP/2 frame (9-byte header, type 0x00-0x09, length < 16MB)
        if ctx.initial_bytes.len() >= 9 {
            let len = u32::from_be_bytes([0, ctx.initial_bytes[0],
                ctx.initial_bytes[1], ctx.initial_bytes[2]]);
            let frame_type = ctx.initial_bytes[3];
            if len < 16_777_216 && frame_type <= 0x09 {
                return Some(DetectionResult {
                    protocol: ProtocolId::new(ProtocolId::HTTP2),
                    confidence: 0.6,
                    method: DetectionMethod::Heuristic,
                    version: Some("h2".into()),
                });
            }
        }

        None
    }
}
```

### 4. `ZmtpDetector` (custom magic-byte)

Detects ZMTP 3.x greeting: byte 0 = `0xFF`, bytes 1-8 = padding, byte 9 = `0x7F`.

```rust
pub struct ZmtpDetector;

impl ProtocolDetector for ZmtpDetector {
    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult> {
        if ctx.transport != TransportLayer::Tcp { return None; }
        if ctx.initial_bytes.len() < 10 { return None; }

        // ZMTP 3.x signature: [0xFF, 8 padding bytes, 0x7F]
        if ctx.initial_bytes[0] == 0xFF && ctx.initial_bytes[9] == 0x7F {
            let version = if ctx.initial_bytes.len() >= 11 {
                format!("3.{}", ctx.initial_bytes[10])
            } else {
                "3.x".into()
            };
            return Some(DetectionResult {
                protocol: ProtocolId::new(ProtocolId::ZMTP),
                confidence: 0.95,
                method: DetectionMethod::MagicBytes,
                version: Some(version),
            });
        }

        None
    }
}
```

### 5. `RtpsDetector` (custom magic-byte)

Detects RTPS header: bytes 0-3 = `"RTPS"`, bytes 4-5 = version.

```rust
pub struct RtpsDetector;

impl ProtocolDetector for RtpsDetector {
    fn detect(&self, ctx: &DetectionContext<'_>) -> Option<DetectionResult> {
        if ctx.transport != TransportLayer::Udp { return None; }
        if ctx.initial_bytes.len() < 4 { return None; }

        if &ctx.initial_bytes[0..4] == b"RTPS" {
            let version = if ctx.initial_bytes.len() >= 6 {
                Some(format!("{}.{}", ctx.initial_bytes[4], ctx.initial_bytes[5]))
            } else {
                None
            };
            return Some(DetectionResult {
                protocol: ProtocolId::new(ProtocolId::RTPS),
                confidence: 0.99,
                method: DetectionMethod::MagicBytes,
                version,
            });
        }

        None
    }
}
```

## Layered Detection Engine

```rust
/// Runs detectors in priority order and returns the first confident match.
pub struct DetectionEngine {
    /// Detectors ordered by priority (highest first).
    detectors: Vec<Box<dyn ProtocolDetector>>,
    /// Minimum confidence to accept a detection result.
    confidence_threshold: f32,
}

impl DetectionEngine {
    pub fn with_defaults() -> Self { /* ... */ }

    /// Detect the protocol for a given context.
    ///
    /// Tries all detectors in priority order. Returns the first result
    /// with confidence >= threshold, or a fallback result.
    pub fn detect(&self, ctx: &DetectionContext<'_>) -> DetectionResult { /* ... */ }
}
```

Default detector order:
1. `PortMappingDetector` (confidence 0.5 — serves as a hint for later detectors)
2. `GrpcDetector` (confidence 0.6-0.95)
3. `ZmtpDetector` (confidence 0.95)
4. `RtpsDetector` (confidence 0.99)
5. `GuessCrateDetector` (confidence 0.85-0.95)

The engine takes the **highest-confidence result** across all detectors, not the
first one that matches. If no detector exceeds the threshold (default 0.5),
return `ProtocolId::UNKNOWN` with `DetectionMethod::Fallback`.

## Tasks

### T1.1: Create `prb-detect` crate skeleton
- Add to workspace `Cargo.toml` members
- Add workspace dependency for `guess`
- Create `src/lib.rs`, `src/detector.rs`, `src/types.rs`
- Define `ProtocolId`, `DetectionResult`, `DetectionContext`, `DetectionMethod`
- Define `ProtocolDetector` trait

### T1.2: Implement `PortMappingDetector`
- Configurable port → protocol table
- Default mappings for gRPC, ZMTP, RTPS well-known ports
- Test: port 50051 → gRPC, port 7400 → RTPS

### T1.3: Implement `GrpcDetector`
- HTTP/2 preface detection (24-byte magic)
- HTTP/2 frame heuristic (9-byte header, valid type)
- Test: real HTTP/2 preface bytes → confidence 0.95
- Test: mid-stream HTTP/2 frame → confidence 0.6
- Test: non-HTTP/2 data → None

### T1.4: Implement `ZmtpDetector`
- ZMTP 3.x greeting detection (0xFF...0x7F)
- Version extraction from byte 10
- Test: ZMTP 3.0 greeting → confidence 0.95
- Test: ZMTP 3.1 greeting → confidence 0.95, version "3.1"
- Test: random data → None

### T1.5: Implement `RtpsDetector`
- RTPS magic bytes "RTPS" + version extraction
- Test: RTPS 2.3 header → confidence 0.99, version "2.3"
- Test: non-RTPS UDP → None

### T1.6: Implement `GuessCrateDetector`
- Wrap `guess` TCP and UDP detectors
- Map `guess::ProtocolInfo` variants to `ProtocolId`
- Test: HTTP/2 preface detected as HTTP2
- Test: TLS Client Hello detected as TLS

### T1.7: Implement `DetectionEngine`
- Priority-ordered detector list
- Highest-confidence-wins selection
- Confidence threshold (configurable, default 0.5)
- Fallback to `UNKNOWN`
- Test: TCP stream with HTTP/2 preface → gRPC (0.95)
- Test: TCP stream with ZMTP greeting → ZMTP (0.95)
- Test: UDP datagram with RTPS header → RTPS (0.99)
- Test: Unknown TCP stream → UNKNOWN (fallback)
- Test: Port 50051 + HTTP/2 preface → gRPC (0.95, magic beats port)

### T1.8: Benchmarks
- Benchmark detection latency for each detector
- Target: <1μs per detection call for all built-in detectors
- Use `criterion` for benchmarking

## Verification

```bash
cargo test -p prb-detect
cargo clippy -p prb-detect -- -D warnings
cargo bench -p prb-detect   # detection latency benchmarks
```

All detection tests pass. No clippy warnings. Detection latency <1μs.
