---
id: "2"
title: "prb-grpc trace context extraction and H2 edge cases untested"
risk: 3/10
addressed_by_segments: [2]
---

# Issue 2: prb-grpc trace context extraction and H2 edge cases untested

## Core Problem

`enrich_with_trace_context()` in decoder.rs (lines 42-68) is never exercised — no test includes `traceparent`/`tracestate` headers. H2 frame edge cases (GoAway type 0x07, RstStream type 0x03, CONTINUATION stream-ID mismatch, HPACK degradation warning on events) are not tested despite existing binary frame-builder helpers in `src/tests.rs`. `LpmParser::from_header` unknown value and `decompress` error paths are also uncovered.

## Root Cause

The existing test suite focuses on happy-path gRPC flows. Edge cases and trace propagation were deferred.

## Proposed Fix

Add ~10 new tests using the existing `create_headers_frame`/`create_data_frame` helpers:
- Request headers with W3C `traceparent` + `tracestate` → assert `otel.trace_id`, `otel.span_id` metadata
- GoAway frame → `decode_stream` returns Ok with no events
- RstStream frame → subsequent DATA on that stream handled gracefully
- HPACK degradation → warnings present on emitted events
- `CompressionAlgorithm::from_header("unknown")` → `Identity`
- Compressed LPM with invalid gzip data → `DecompressionError`

## Existing Solutions Evaluated

N/A — internal test additions using existing test helpers.

## Pre-Mortem

- HPACK degradation is hard to trigger synthetically — may need to craft a header block that references a dynamic table entry that was never added.
- Trace context parsing depends on `prb-core::trace::extract_trace_context` — ensure header key casing matches expectations.

## Risk Factor: 3/10

Uses existing infrastructure, well-bounded scope.

## Blast Radius

- Direct: `crates/prb-grpc/src/tests.rs` (or new test file)
- Ripple: None
