---
segment: 13
title: "Real-Data Tests: gRPC/HTTP2 Protocol Captures"
depends_on: [5, 11]
risk: 4
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "test(prb-grpc,prb-pcap): add real-data integration tests with gRPC/HTTP2 captures"
---

# Segment 13: Real-Data Tests — gRPC/HTTP2 Protocol Captures

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Download real-world gRPC and HTTP/2 pcap captures, add them as test fixtures, and write integration tests that parse them end-to-end through the prb pipeline.

**Depends on:** Segments 5 (grpc H2 parser tests), 11 (integration tests)

## Data Sources

Download the following captures (all publicly available):

### From Wireshark GitLab / Wiki
1. **grpc_person_search_protobuf_with_image.pcapng** — gRPC with protobuf, Person Search service
   - URL: `https://wiki.wireshark.org/uploads/__moin_import__/attachments/gRPC/grpc_person_search_protobuf_with_image.pcapng`
2. **http2-h2c.pcap** — HTTP/2 cleartext (h2c) via Upgrade
   - URL: `https://wiki.wireshark.org/uploads/__moin_import__/attachments/HTTP2/http2-h2c.pcap`
3. **http2-16-ssl.pcapng** — HTTP/2 over TLS with key material
   - URL: `https://wiki.wireshark.org/uploads/__moin_import__/attachments/HTTP2/http2-16-ssl.pcapng`

### From GitHub repos
4. **salrashid123/grpc_sslkeylog** — gRPC TLS capture with SSLKEYLOGFILE
   - Repo: `https://github.com/salrashid123/grpc_sslkeylog`
   - Look for pcap + keylog pairs in the repo

### From Wireshark gRPC issue (GitLab)
5. Various gRPC captures attached to issue #13932:
   - `grpc_hello2_1call_very_simple2_gzip_javacs.pcapng`
   - `grpc_json_gzip_helloworld2_javaclientsserver.pcapng`
   - `grpc_json_streamtest.pcapng`

Store fixtures in `tests/fixtures/captures/grpc/` and `tests/fixtures/captures/http2/`.

## Scope

- `tests/fixtures/captures/grpc/*.pcapng` — Downloaded fixture files
- `tests/fixtures/captures/http2/*.pcap` — Downloaded fixture files
- `crates/prb-grpc/tests/real_data_tests.rs` — New test file
- `crates/prb-pcap/tests/real_data_grpc_tests.rs` — Pipeline integration with real captures

## Implementation Approach

### Download and curate fixtures
- Use `curl` to download each capture file
- Verify file integrity (check file size > 0, pcap header valid)
- Document each fixture's source URL in a `README.md` in the fixtures directory
- Git LFS is NOT needed — these files are typically 10KB-5MB

### Test suite: prb-grpc real data tests
```rust
#[test]
fn test_grpc_person_search_decode() {
    // Read pcapng → normalize → reassemble → H2 parse → gRPC decode
    // Assert: at least N gRPC requests/responses found
    // Assert: method names match expected service definition
    // Assert: protobuf payloads are non-empty
}

#[test]
fn test_grpc_streaming_decode() {
    // Use grpc_json_streamtest.pcapng
    // Assert: streaming messages are correctly framed
    // Assert: multiple messages per stream
}

#[test]
fn test_h2c_cleartext_decode() {
    // Use http2-h2c.pcap
    // Assert: HTTP/2 frames parsed without TLS
    // Assert: HEADERS + DATA frames present
}
```

### Test suite: full pipeline with real gRPC captures
```rust
#[test]
fn test_full_pipeline_grpc_capture() {
    // Read pcapng → PipelineCore → DebugEvents
    // Assert: events have protocol "grpc"
    // Assert: events have valid src/dst addresses
    // Assert: metadata contains grpc-method, grpc-status
}
```

### Test with TLS (if keylog available)
```rust
#[test]
fn test_grpc_tls_with_keylog() {
    // Read encrypted capture + keylog file
    // Assert: TLS decryption succeeds
    // Assert: Decrypted gRPC messages are valid
}
```

## Pre-Mortem Risks

- Some captures may use HTTP/2 draft versions that differ from final spec
- gRPC captures with custom protobuf schemas won't have schema-backed decode — test wire-format decode instead
- Large captures may slow tests — use `#[ignore]` for captures > 1MB and run separately

## Build and Test Commands

- Build: `cargo check -p prb-grpc -p prb-pcap`
- Test (targeted): `cargo nextest run -p prb-grpc -p prb-pcap -E 'test(real_data)'`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** At least 6 tests using real gRPC/HTTP2 captures, all passing
2. **Fixture files:** At least 4 real-world capture files committed to tests/fixtures/captures/
3. **Documentation:** README.md in fixtures directory with source URLs and licenses
4. **Regression tests:** `cargo nextest run --workspace` — no regressions
5. **Full build gate:** `cargo build --workspace`
6. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
7. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
