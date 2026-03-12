---
segment: 2
title: "gRPC trace context + H2 edge cases"
depends_on: []
risk: 3
complexity: Low
cycle_budget: 10
status: pending
commit_message: "test(prb-grpc): add trace context extraction, GoAway, RstStream, HPACK degradation tests"
---

# Segment 2: gRPC trace context + H2 edge cases

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Push prb-grpc from 72.3% to 90%+ by testing trace context extraction, H2 frame edge cases, and LPM error paths using the existing binary frame-builder test helpers.

**Depends on:** None

## Issues Addressed

Issue 2 — prb-grpc missing trace context extraction and H2 edge case tests.

## Scope

- `crates/prb-grpc/src/decoder.rs` — `enrich_with_trace_context`, `process_h2_events` edge branches
- `crates/prb-grpc/src/h2.rs` — GoAway, RstStream, CONTINUATION errors, HPACK degradation
- `crates/prb-grpc/src/lpm.rs` — `from_header` unknown, decompress error
- `crates/prb-grpc/src/correlation.rs` — `grouping_key` with `network: None`

## Key Files and Context

**Existing test infrastructure in `crates/prb-grpc/src/tests.rs`:** Contains helpers `create_settings_frame`, `create_headers_frame`, `create_data_frame`, `create_window_update_frame`. These construct raw HTTP/2 binary frames. Tests call `decoder.decode_stream(&data, &ctx)` and assert on returned `DebugEvent` fields.

**`enrich_with_trace_context` (decoder.rs ~42-68):** Called from `create_trailers_event` and `create_message_event`. Extracts W3C `traceparent`, `tracestate`, B3 single/multi headers. Adds `otel.trace_id`, `otel.span_id` to event metadata and creates a `CorrelationKey::TraceContext`. Currently never exercised because no test includes trace headers.

**H2 frame types to test:**
- GoAway (type 0x07): `[0x00, 0x00, payload_len, 0x07, 0x00, 0x00, 0x00, 0x00, 0x00, ...last_stream_id, error_code]`
- RstStream (type 0x03): `[0x00, 0x00, 0x04, 0x03, 0x00, stream_id_bytes, error_code_bytes]`

**LPM `CompressionAlgorithm::from_header`:** Maps `"gzip"` → Gzip, `"deflate"` → Zlib, `"identity"` → Identity, anything else → Identity.

## Implementation Approach

1. Add test `test_grpc_traceparent_extraction`: build HEADERS frame with `:method POST`, `:path /pkg.Svc/Method`, `traceparent: 00-{trace_id}-{span_id}-01`. Follow with DATA frame. Assert emitted event has `metadata["otel.trace_id"]` and `metadata["otel.span_id"]`.
2. Add test `test_grpc_tracestate_propagation`: same as above plus `tracestate: congo=t61rcWkgMzE`. Assert `metadata["otel.tracestate"]` present.
3. Add test `test_grpc_go_away_frame`: send SETTINGS + GoAway frame. Assert `decode_stream` returns Ok with no events (GoAway is a no-op in the decoder).
4. Add test `test_grpc_rst_stream`: send HEADERS + RstStream for same stream. Verify no panic.
5. Add test `test_hpack_dynamic_table_miss`: craft a header block referencing dynamic table index that doesn't exist. Assert HPACK degradation warning or graceful fallback.
6. Add test `test_lpm_from_header_unknown`: `CompressionAlgorithm::from_header("br")` returns `Identity`.
7. Add test `test_lpm_decompress_invalid_gzip`: create LPM parser with Gzip, feed valid LPM header + random bytes. Assert `DecompressionError`.
8. Add test `test_correlation_grouping_key_no_network`: event with `source.network = None`, verify key contains "unknown".

## Alternatives Ruled Out

- Adding real pcap fixture tests: overkill for branch coverage, existing helpers are sufficient.
- Mocking H2 codec internally: not needed, binary frame construction is straightforward.

## Pre-Mortem Risks

- `traceparent` header must be lowercase in HPACK literal encoding — ensure the helper correctly emits it.
- GoAway frame format: the frame body is `last_stream_id (4 bytes) + error_code (4 bytes)` — minimum 8 bytes payload.
- HPACK degradation test: may require understanding the exact code path in `parse_hpack_headers` that sets `hpack_degraded = true`. Look for dynamic table index references (byte starting with 0x80 + index).

## Build and Test Commands

- Build: `cargo build -p prb-grpc`
- Test (targeted): `cargo test -p prb-grpc -- trace && cargo test -p prb-grpc -- go_away && cargo test -p prb-grpc -- rst_stream && cargo test -p prb-grpc -- lpm`
- Test (regression): `cargo test -p prb-grpc`
- Test (full gate): `cargo test -p prb-grpc`

## Exit Criteria

1. **Targeted tests:**
   - `test_grpc_traceparent_extraction`: event metadata has `otel.trace_id` and `otel.span_id`
   - `test_grpc_tracestate_propagation`: event metadata has `otel.tracestate`
   - `test_grpc_go_away_frame`: decode returns Ok, no panic
   - `test_grpc_rst_stream`: decode returns Ok, no panic
   - `test_lpm_from_header_unknown`: returns Identity
   - `test_lpm_decompress_invalid_gzip`: returns DecompressionError
   - `test_correlation_grouping_key_no_network`: key contains "unknown"
2. **Regression tests:** All 25 existing prb-grpc tests pass
3. **Full build gate:** `cargo build -p prb-grpc`
4. **Full test gate:** `cargo test -p prb-grpc`
5. **Self-review gate:** No dead code, only test additions
6. **Scope verification gate:** Only `crates/prb-grpc/src/tests.rs` (or new test file) modified

**Risk factor:** 3/10
**Estimated complexity:** Low
