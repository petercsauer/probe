---
segment: 23
title: "Real-Data Tests: OTel Trace Correlation with Network Captures"
depends_on: [13, 22]
risk: 4
complexity: High
cycle_budget: 5
status: pending
commit_message: "test(prb-core,prb-otel): add real-data OTel trace correlation tests with gRPC/HTTP captures"
---

# Segment 23: Real-Data Tests — OTel Trace Correlation with Network Captures

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Create test fixtures that combine network captures with matching OpenTelemetry trace/span data. Verify the OTel correlation engine can link network events to distributed traces.

**Depends on:** Segments 13 (gRPC real data), 22 (conversation + export tests)

## Data Sources

### Self-generated captures with OTel instrumentation
Since public captures with matching OTel data are extremely rare, generate test fixtures:

1. **gRPC service with OTel tracing**:
   ```bash
   # Run a simple instrumented gRPC service
   # Capture both: network traffic (pcap) and OTel spans (JSON)
   # The traceparent/tracestate headers in gRPC metadata link the two
   ```

2. **HTTP service with W3C Trace Context**:
   ```bash
   # HTTP requests with traceparent header
   # Capture network traffic + OTel span export
   # traceparent header format: 00-{trace_id}-{span_id}-{flags}
   ```

### Synthetic fixture generation
3. **Create paired fixtures programmatically**:
   - Generate a pcap with HTTP requests containing traceparent headers
   - Generate matching OTel span JSON with the same trace_id/span_id values
   - This guarantees correlation can be tested deterministically

### From OpenTelemetry demo app
4. **OTel Astronomy Shop demo** — `https://github.com/open-telemetry/opentelemetry-demo`
   - Multi-service demo with gRPC + HTTP + tracing
   - Could capture network traffic while running the demo
   - Span data available from OTel Collector export

Store fixtures in `tests/fixtures/captures/otel/` with paired `.pcap` + `.spans.json` files.

## Scope

- `tests/fixtures/captures/otel/*.pcap` — Network captures with trace context headers
- `tests/fixtures/captures/otel/*.spans.json` — Matching OTel span exports
- `crates/prb-core/tests/real_data_otel_correlation_tests.rs` — New test file

## Implementation Approach

### W3C Trace Context extraction from HTTP
```rust
#[test]
fn test_traceparent_header_extraction() {
    // Load HTTP capture containing traceparent headers
    // Run through pipeline
    // Assert: trace_id extracted from traceparent header
    // Assert: span_id extracted
    // Assert: trace flags parsed
}

#[test]
fn test_tracestate_header_extraction() {
    // Load capture with tracestate header (vendor-specific propagation)
    // Assert: tracestate key-value pairs parsed
}
```

### gRPC metadata trace context
```rust
#[test]
fn test_grpc_metadata_trace_extraction() {
    // Load gRPC capture with grpc-trace-bin or traceparent in metadata
    // Assert: trace context extracted from gRPC headers
}
```

### Correlation engine tests
```rust
#[test]
fn test_correlate_network_events_to_spans() {
    // Load pcap + matching spans.json
    // Run correlation engine
    // Assert: network events linked to their OTel spans
    // Assert: trace_id matches between network event and span
    // Assert: timing overlap between network event and span
}

#[test]
fn test_correlation_with_missing_spans() {
    // Load pcap but only partial span data
    // Assert: uncorrelated events remain valid (no panic)
    // Assert: correlated events have span references, uncorrelated don't
}

#[test]
fn test_correlation_with_multiple_services() {
    // Load capture from multi-service interaction
    // Assert: same trace_id appears across different src/dst pairs
    // Assert: parent-child span relationships reflected in network flow
}
```

### Synthetic fixture generation (build step)
```rust
fn generate_otel_test_fixtures() {
    // Programmatically create:
    // 1. A minimal pcap with HTTP GET containing traceparent header
    // 2. Matching spans.json with the same trace_id/span_id
    // This avoids dependency on external services
}

#[test]
fn test_synthetic_otel_correlation() {
    // Use generated fixtures
    // Assert: 100% correlation rate for matched fixtures
}
```

### End-to-end: capture → correlate → export as OTLP
```rust
#[test]
fn test_capture_to_otlp_export_with_correlation() {
    // Load pcap + spans → correlate → export as OTLP
    // Assert: exported OTLP spans include network-derived attributes
    // Assert: original span context preserved
    // Assert: network timing enriches span data
}
```

## Pre-Mortem Risks

- Public captures with OTel trace headers are nearly non-existent — synthetic generation is essential
- gRPC trace context may be in binary format (grpc-trace-bin) — need binary metadata parsing
- Timing correlation between pcap timestamps and OTel span timestamps may have clock skew
- The OTel correlation engine may not exist yet — if so, test the extraction layer only

## Build and Test Commands

- Build: `cargo check -p prb-core`
- Test (targeted): `cargo nextest run -E 'test(real_data_otel_correlation)'`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** At least 6 OTel correlation tests, all passing
2. **Fixture files:** At least 2 pcap+spans.json paired fixtures
3. **Trace context extraction:** W3C traceparent header correctly parsed from HTTP and gRPC
4. **Correlation accuracy:** 100% match rate on synthetic fixtures
5. **Graceful degradation:** Missing spans don't crash correlator
6. **Regression tests:** `cargo nextest run --workspace` — no regressions
7. **Full build gate:** `cargo build --workspace`
8. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
9. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
