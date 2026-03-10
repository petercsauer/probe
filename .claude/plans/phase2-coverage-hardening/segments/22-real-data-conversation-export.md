---
segment: 22
title: "Real-Data Tests: Conversation Reconstruction and Export Validation"
depends_on: [13, 17, 18]
risk: 3
complexity: Medium
cycle_budget: 5
status: pending
commit_message: "test(prb-core,prb-export): add real-data conversation reconstruction and export format tests"
---

# Segment 22: Real-Data Tests — Conversation Reconstruction and Export Validation

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Use real-world captures from prior segments to test the conversation reconstruction engine and all export formats (CSV, HAR, OTLP) end-to-end. Validate that exported data is spec-compliant and round-trippable.

**Depends on:** Segments 13 (gRPC/HTTP2 real data), 17 (HTTP real data), 18 (SMB/enterprise real data)

## Data Sources

Re-use captures from prior segments:
1. **HTTP/1.1 captures** (from segment 17) — `tests/fixtures/captures/http/http.cap`
2. **gRPC captures** (from segment 13) — `tests/fixtures/captures/grpc/grpc_person_search*.pcapng`
3. **DNS captures** (from segment 16) — `tests/fixtures/captures/dns/dns.cap`
4. **SMB captures** (from segment 18) — `tests/fixtures/captures/smb/smb2-peter.pcap`
5. **Mixed-traffic captures** (from segment 21) — adversarial captures with multi-protocol traffic

No new fixture downloads needed — this segment tests the output side of the pipeline using inputs from earlier segments.

## Scope

- `crates/prb-core/tests/real_data_conversation_tests.rs` — New test file
- `crates/prb-export/tests/real_data_export_tests.rs` — New test file
- Depends on fixtures already downloaded by segments 13-21

## Implementation Approach

### Conversation reconstruction from real captures
```rust
#[test]
fn test_http_conversation_reconstruction() {
    // Load HTTP capture → full pipeline → conversation engine
    // Assert: at least 1 conversation reconstructed
    // Assert: request-response pairs correctly matched
    // Assert: conversation has valid timing (start < end)
}

#[test]
fn test_grpc_conversation_reconstruction() {
    // Load gRPC capture → pipeline → conversations
    // Assert: gRPC method call is a conversation
    // Assert: streaming calls have multiple messages in one conversation
}

#[test]
fn test_dns_conversation_reconstruction() {
    // Load DNS capture → pipeline → conversations
    // Assert: query-response pairs matched by transaction ID
    // Assert: conversation duration = response_ts - query_ts
}

#[test]
fn test_multi_protocol_conversations() {
    // Load mixed-traffic capture
    // Assert: conversations grouped correctly by protocol
    // Assert: no cross-protocol contamination
}
```

### CSV export with real data
```rust
#[test]
fn test_csv_export_from_http_capture() {
    // HTTP capture → pipeline → CSV export
    // Assert: CSV is valid (parseable by csv crate)
    // Assert: column headers present
    // Assert: row count matches event count
    // Assert: timestamp, src, dst, protocol columns populated
}

#[test]
fn test_csv_export_field_escaping() {
    // Use capture with data containing commas, quotes, newlines
    // Assert: CSV properly escapes special characters
}
```

### HAR export with real data
```rust
#[test]
fn test_har_export_from_http_capture() {
    // HTTP capture → pipeline → HAR export
    // Assert: Valid HAR JSON (parseable, matches HAR 1.2 schema)
    // Assert: entries have request + response objects
    // Assert: timing fields populated
    // Assert: headers present in request/response
}

#[test]
fn test_har_export_from_grpc_capture() {
    // gRPC capture → pipeline → HAR export
    // Assert: gRPC calls represented as HAR entries
    // Assert: HTTP/2 pseudo-headers (:method, :path, :status) mapped correctly
}

#[test]
fn test_har_roundtrip_validity() {
    // Export to HAR → parse back → verify structure
    // Use serde_json::from_str::<serde_json::Value>
    // Assert: all required HAR fields present
}
```

### OTLP export with real data
```rust
#[test]
fn test_otlp_export_from_http_capture() {
    // HTTP capture → pipeline → OTLP span export
    // Assert: spans have valid trace_id, span_id
    // Assert: span timing matches event timing
    // Assert: attributes include http.method, http.status_code
}

#[test]
fn test_otlp_export_parent_child_spans() {
    // gRPC capture → pipeline → OTLP export
    // Assert: parent-child span relationships for request/response
    // Assert: trace_id consistent within a conversation
}
```

### Cross-format consistency
```rust
#[test]
fn test_csv_and_har_event_count_match() {
    // Same capture → export to both CSV and HAR
    // Assert: same number of events/entries in both
}
```

## Pre-Mortem Risks

- This segment depends on fixtures from segments 13-21 existing — if run before those, skip gracefully
- HAR 1.2 schema validation may require pulling in a JSON schema validator
- OTLP span format may not perfectly map from all protocol types
- Conversation reconstruction accuracy depends on protocol decoder quality

## Build and Test Commands

- Build: `cargo check -p prb-core -p prb-export`
- Test (targeted): `cargo nextest run -E 'test(real_data_conversation) | test(real_data_export)'`
- Test (regression): `cargo nextest run --workspace`
- Test (full gate): `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`

## Exit Criteria

1. **Targeted tests:** At least 10 tests covering conversation reconstruction + all 3 export formats
2. **Conversation accuracy:** Request-response pairing correct for HTTP, gRPC, DNS
3. **CSV validity:** Exported CSV parseable by standard CSV parser
4. **HAR validity:** Exported HAR matches HAR 1.2 schema structure
5. **OTLP validity:** Exported spans have valid trace_id, span_id, timing
6. **Cross-format consistency:** Event counts match across export formats
7. **Regression tests:** `cargo nextest run --workspace` — no regressions
8. **Full build gate:** `cargo build --workspace`
9. **Full test gate:** `cargo nextest run --workspace && cargo clippy --workspace -- -D warnings`
10. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks
