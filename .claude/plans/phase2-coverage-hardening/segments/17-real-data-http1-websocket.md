---
segment: 17
title: "Real-Data Tests: HTTP/1.x and WebSocket Captures"
depends_on: [11]
risk: 2
complexity: Low
cycle_budget: 3
status: pending
commit_message: "test(prb-pcap,prb-decode): add real-data HTTP/1.x and WebSocket protocol tests"
---

# Segment 17: Real-Data Tests — HTTP/1.x and WebSocket Captures

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Download real-world HTTP/1.0, HTTP/1.1, and WebSocket captures and test the decode pipeline against them.

**Depends on:** Segment 11 (integration tests)

## Data Sources

### From Wireshark Wiki SampleCaptures
1. **http.cap** — Basic HTTP/1.1 request/response
   - URL: `https://wiki.wireshark.org/uploads/__moin_import__/attachments/SampleCaptures/http.cap`
2. **http_gzip.cap.gz** — HTTP with gzip content-encoding
3. **http-chunked-gzip.pcap** — HTTP chunked transfer with gzip
4. **http_with_jpegs.cap.gz** — HTTP transferring JPEG images (large payloads)
5. **websocket.cap** — WebSocket handshake and data frames (if available on wiki)

### From http2/http_samples (GitHub)
6. Archived HTTP header samples
   - Repo: `https://github.com/http2/http_samples`

### From Malware Traffic Analysis
7. HTTP-based malware traffic (tests robustness)
   - `https://malware-traffic-analysis.net/` — recent captures with HTTP traffic
   - Tests parser against adversarial/unusual HTTP patterns

### Self-generated WebSocket (if no public captures found)
```bash
# Generate WebSocket capture with wscat + tcpdump
tcpdump -i lo0 port 8080 -w ws_test.pcap &
wscat -c ws://localhost:8080
```

Store fixtures in `tests/fixtures/captures/http/` and `tests/fixtures/captures/websocket/`.

## Scope

- `tests/fixtures/captures/http/*.pcap` — HTTP/1.x captures
- `tests/fixtures/captures/websocket/*.pcap` — WebSocket captures
- `crates/prb-pcap/tests/real_data_http_tests.rs` — New test file
- `crates/prb-decode/tests/real_data_http_tests.rs` — Decode-level tests

## Implementation Approach

### HTTP/1.1 decode tests
```rust
#[test]
fn test_http11_request_response_real() {
    // Load http.cap
    // Run through normalize → reassemble → decode
    // Assert: HTTP methods (GET, POST) found
    // Assert: Status codes (200, 301, 404) found
    // Assert: Headers parsed (Host, Content-Type, etc.)
}

#[test]
fn test_http_gzip_content_encoding() {
    // Load http_gzip.cap
    // Assert: Content-Encoding: gzip detected
    // Assert: Body decompressed correctly (or at least detected)
}

#[test]
fn test_http_chunked_transfer() {
    // Load http-chunked-gzip.pcap
    // Assert: chunked transfer-encoding handled
    // Assert: reassembled body is complete
}

#[test]
fn test_http_large_payload() {
    // Load http_with_jpegs.cap
    // Assert: large payloads don't crash the pipeline
    // Assert: Content-Length matches reassembled data length
}
```

### WebSocket decode tests
```rust
#[test]
fn test_websocket_handshake() {
    // Load WebSocket capture
    // Assert: HTTP upgrade request detected
    // Assert: Sec-WebSocket-Accept header present in response
    // Assert: Connection transitions from HTTP to WebSocket
}

#[test]
fn test_websocket_data_frames() {
    // Assert: Text and binary frames decoded
    // Assert: Frame masking handled (client → server)
    // Assert: Fragmented frames reassembled
}
```

### HTTP edge cases
```rust
#[test]
fn test_http_pipelined_requests() {
    // Multiple HTTP requests on same TCP connection
    // Assert: each request/response pair separated correctly
}

#[test]
fn test_http_malformed_headers() {
    // Load malware-traffic capture with malformed HTTP
    // Assert: parser doesn't panic
    // Assert: best-effort decode produces events
}
```

## Pre-Mortem Risks

- HTTP/1.0 vs 1.1 connection semantics differ (keep-alive default) — verify both
- Chunked encoding + compression combinations may stress the pipeline
- WebSocket captures may be rare in public datasets — may need self-generated captures

## Build and Test Commands

- Build: `cargo check -p prb-pcap -p prb-decode`
- Test (targeted): `cargo nextest run -E 'test(real_data_http) | test(real_data_websocket)'`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** At least 6 tests with real HTTP/WebSocket captures, all passing
2. **Fixture files:** At least 4 HTTP + 1 WebSocket capture files committed
3. **Protocol coverage:** HTTP/1.1 basic, gzip, chunked, large payload, WebSocket handshake + data
4. **Regression tests:** `cargo nextest run --workspace` — no regressions
5. **Full build gate:** `cargo build --workspace`
6. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
7. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
