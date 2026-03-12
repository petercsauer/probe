# ADR 0003: Protocol Detection Engine

## Status

Accepted

## Context

PRB needs to automatically detect which protocol decoder to use for each TCP stream
or UDP flow. Users should not need to manually specify protocols for most captures.

## Decision

Implement a multi-stage detection pipeline with confidence scoring:

1. **Port mapping detector**: Fast static lookup (50051 → gRPC, 7400-7500 → RTPS)
2. **Magic bytes detector**: Protocol-specific signatures (HTTP/2 preface, ZMTP greeting, RTPS header)
3. **Heuristic detector**: Structural analysis (HTTP/2 frame format, message patterns)
4. **Fallback**: Best-effort wire-format decode or raw bytes

Detection returns a confidence score (0.0-1.0). The first detector with confidence ≥ 0.8 wins.

## Consequences

**Positive:**
- Works automatically for most captures
- Extensible via plugin detectors
- Fast (port lookup is O(1), magic bytes are first few bytes)
- Graceful degradation when detection fails

**Negative:**
- False positives possible with heuristic detection
- Port-based detection fails with non-standard ports (requires manual override)
- Some protocols hard to distinguish (HTTP/2 vs gRPC over HTTP/2)

## Implementation

The `prb-detect` crate provides:

- `ProtocolDetector` trait for implementing detection logic
- `DetectionEngine` for orchestrating multiple detectors
- Built-in detectors for gRPC/HTTP/2, ZMTP, and RTPS
- Port mapping configuration via YAML

Example detection flow:

```rust
let context = DetectionContext {
    initial_bytes: &stream[..256],
    src_port: 50051,
    dst_port: 12345,
    transport: TransportLayer::Tcp,
    tls_decrypted: true,
};

let result = engine.detect(&context);
// Result: ProtocolId("grpc"), confidence: 0.95, method: MagicBytes
```

## Future Considerations

- Add content-based detection for mid-stream captures
- Support protocol version detection
- Learn from user corrections to improve heuristics
