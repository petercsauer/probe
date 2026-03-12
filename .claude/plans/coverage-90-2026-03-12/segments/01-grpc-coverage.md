---
segment: 01
title: prb-grpc to 90%
depends_on: []
risk: 3
complexity: Medium
cycle_budget: 8
estimated_lines: ~200 test lines
---

# Segment 01: prb-grpc Coverage to 90%

## Context

**Current coverage:** 72.69% (72.09% regions)
**Target coverage:** 90%
**Gap:** +17.31 percentage points (~228 uncovered lines)

prb-grpc is the gRPC/HTTP/2 protocol decoder. Main gap is in HTTP/2 frame parsing.

**Module breakdown:**
- `src/h2.rs` - **46.83% (CRITICAL GAP - 176 lines uncovered)**
- `src/decoder.rs` - 81.36% (42 lines uncovered)
- `src/correlation.rs` - 90.00% (7 lines uncovered)
- `src/lpm.rs` - 94.83% (3 lines uncovered)

**Existing tests:**
- 25 test functions in `src/tests.rs` (inline tests)
- 4 tests in `tests/real_data_fixture_tests.rs` (fixture validation)
- Good coverage of happy paths and basic gRPC operations

## Goal

Add comprehensive tests for HTTP/2 frame parsing and edge cases to bring prb-grpc from 72.69% to 90%.

## Exit Criteria

1. [ ] prb-grpc crate coverage ≥90% (verified with `cargo llvm-cov -p prb-grpc --summary-only`)
2. [ ] `src/h2.rs` coverage ≥85%
3. [ ] `src/decoder.rs` coverage ≥88%
4. [ ] All new tests pass (`cargo test -p prb-grpc`)
5. [ ] No regression in existing 29 tests
6. [ ] Tests cover: frame type variations, HPACK edge cases, stream state transitions, error paths

## Implementation Plan

### Priority 1: h2.rs HTTP/2 Frame Parsing (~150 lines of tests)

**Current state:** 470 regions, 245 uncovered (47.87%)

**Specific gaps identified:**
- Frame type handling (lines 141-256): DATA, HEADERS, CONTINUATION, RST_STREAM, SETTINGS, GOAWAY
- HPACK decoding (lines 258-406): indexed headers, literal headers, dynamic table references, multi-byte integers
- Static table lookup (lines 444-528): HTTP/2 static table entries
- Error paths: invalid frame lengths, unknown frame types, CONTINUATION mismatches

**Add tests:**

```rust
// crates/prb-grpc/src/tests.rs (add to existing test module)

#[test]
fn test_h2_data_frame_with_end_stream() {
    let mut codec = H2Codec::new();
    // Feed HTTP/2 preface
    let preface = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";
    codec.process(preface).unwrap();

    // Create DATA frame with END_STREAM flag
    let mut frame = vec![0, 0, 5, 0x00, 0x01]; // length=5, type=DATA, flags=END_STREAM
    frame.extend_from_slice(&1u32.to_be_bytes()); // stream_id=1
    frame.extend_from_slice(b"hello");

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);
    if let H2Event::Data { stream_id, data, end_stream } = &events[0] {
        assert_eq!(*stream_id, 1);
        assert_eq!(data.as_ref(), b"hello");
        assert!(*end_stream);
    } else {
        panic!("Expected Data event");
    }
}

#[test]
fn test_h2_headers_frame_simple() {
    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // HEADERS frame with END_HEADERS flag, literal header without indexing
    let payload = vec![
        0x00, // Literal without indexing, name index = 0
        0x04, b':','p','a','t','h', // name length + name
        0x01, b'/',  // value length + value
    ];
    let mut frame = vec![
        0, 0, payload.len() as u8,
        0x01, // type=HEADERS
        0x04, // flags=END_HEADERS
    ];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);
    if let H2Event::Headers { stream_id, headers, .. } = &events[0] {
        assert_eq!(*stream_id, 1);
        assert_eq!(headers.get(":path").unwrap(), "/");
    } else {
        panic!("Expected Headers event");
    }
}

#[test]
fn test_h2_continuation_frame() {
    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // HEADERS frame without END_HEADERS
    let payload1 = vec![0x00, 0x04, b':','p','a','t','h', 0x01, b'/'];
    let mut frame1 = vec![0, 0, payload1.len() as u8, 0x01, 0x00]; // No END_HEADERS
    frame1.extend_from_slice(&1u32.to_be_bytes());
    frame1.extend_from_slice(&payload1);

    // CONTINUATION frame with END_HEADERS
    let payload2 = vec![0x00, 0x07, b':','m','e','t','h','o','d', 0x03, b'G','E','T'];
    let mut frame2 = vec![0, 0, payload2.len() as u8, 0x09, 0x04]; // CONTINUATION + END_HEADERS
    frame2.extend_from_slice(&1u32.to_be_bytes());
    frame2.extend_from_slice(&payload2);

    codec.process(&frame1).unwrap(); // No events yet
    let events = codec.process(&frame2).unwrap();
    assert_eq!(events.len(), 1);
    if let H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(headers.get(":path").unwrap(), "/");
        assert_eq!(headers.get(":method").unwrap(), "GET");
    } else {
        panic!("Expected Headers event");
    }
}

#[test]
fn test_h2_rst_stream_frame() {
    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // RST_STREAM frame
    let mut frame = vec![0, 0, 4, 0x03, 0x00]; // length=4, type=RST_STREAM
    frame.extend_from_slice(&5u32.to_be_bytes()); // stream_id=5
    frame.extend_from_slice(&0x08u32.to_be_bytes()); // error_code=CANCEL

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);
    matches!(events[0], H2Event::RstStream { stream_id: 5 });
}

#[test]
fn test_h2_settings_frame() {
    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // SETTINGS frame (empty settings)
    let frame = vec![0, 0, 0, 0x04, 0x00, 0, 0, 0, 0]; // length=0, type=SETTINGS, stream_id=0

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);
    matches!(events[0], H2Event::Settings);
}

#[test]
fn test_h2_goaway_frame() {
    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // GOAWAY frame
    let mut frame = vec![0, 0, 8, 0x07, 0x00]; // length=8, type=GOAWAY
    frame.extend_from_slice(&0u32.to_be_bytes()); // stream_id=0 (connection-level)
    frame.extend_from_slice(&10u32.to_be_bytes()); // last_stream_id=10
    frame.extend_from_slice(&0x01u32.to_be_bytes()); // error_code=PROTOCOL_ERROR

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);
    matches!(events[0], H2Event::GoAway);
}

#[test]
fn test_h2_unknown_frame_type_skipped() {
    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Unknown frame type 0xFF
    let frame = vec![0, 0, 0, 0xFF, 0x00, 0, 0, 0, 1];

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 0); // Unknown frames are skipped
}

#[test]
fn test_h2_partial_frame_buffering() {
    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // Send partial frame header
    let partial = vec![0, 0, 10]; // Only 3 bytes of 9-byte header
    let events = codec.process(&partial).unwrap();
    assert_eq!(events.len(), 0); // Buffered, no events yet

    // Send rest of header + payload
    let rest = vec![
        0x00, 0x01, // Complete header: type=DATA, flags=END_STREAM
        0, 0, 0, 1,  // stream_id=1
        b'h','e','l','l','o','w','o','r','l','d', // 10 bytes payload
    ];
    let events = codec.process(&rest).unwrap();
    assert_eq!(events.len(), 1);
}

#[test]
fn test_h2_indexed_header_from_static_table() {
    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // HEADERS with indexed header (static table index 2 = :method GET)
    let payload = vec![0x82]; // Indexed header field, index=2
    let mut frame = vec![0, 0, 1, 0x01, 0x05]; // END_STREAM + END_HEADERS
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    assert_eq!(events.len(), 1);
    if let H2Event::Headers { headers, .. } = &events[0] {
        assert_eq!(headers.get(":method").unwrap(), "GET");
    } else {
        panic!("Expected Headers event");
    }
}

#[test]
fn test_h2_hpack_degradation_warning() {
    let mut codec = H2Codec::new();
    codec.process(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").unwrap();

    // HEADERS with dynamic table reference (index > 61, not in static table)
    let payload = vec![0xBE]; // Indexed header field, index=62 (dynamic table)
    let mut frame = vec![0, 0, 1, 0x01, 0x05];
    frame.extend_from_slice(&1u32.to_be_bytes());
    frame.extend_from_slice(&payload);

    let events = codec.process(&frame).unwrap();
    // Should get HpackDegraded event + empty Headers
    assert!(events.iter().any(|e| matches!(e, H2Event::HpackDegraded { .. })));
}
```

### Priority 2: decoder.rs Edge Cases (~50 lines of tests)

**Current state:** 338 regions, 63 uncovered (81.36%)

**Add tests for gRPC message framing:**

```rust
#[test]
fn test_grpc_message_length_prefix_split() {
    use crate::decoder::GrpcDecoder;
    use prb_core::{DecodeContext, ProtocolDecoder};

    let mut decoder = GrpcDecoder::new();
    let ctx = DecodeContext::new()
        .with_src_addr("127.0.0.1:50051")
        .with_dst_addr("127.0.0.1:12345");

    // Simulate gRPC message split across TCP segments
    let part1 = vec![0x00]; // Compression flag
    let part2 = vec![0x00, 0x00, 0x00, 0x05]; // Length = 5
    let part3 = b"hello".to_vec(); // Payload

    // Feed incrementally
    decoder.decode_stream(&part1, &ctx).unwrap(); // No event yet
    decoder.decode_stream(&part2, &ctx).unwrap(); // No event yet
    let events = decoder.decode_stream(&part3, &ctx).unwrap();
    assert!(!events.is_empty());
}

#[test]
fn test_grpc_compressed_message_flag() {
    // Test gRPC compression flag=1
    // Current coverage gap: compression handling paths
}

#[test]
fn test_grpc_multiple_messages_in_stream() {
    // Test multiple gRPC messages in single DATA frame
    // Current coverage gap: message boundary detection
}
```

## Files to Modify

- `crates/prb-grpc/src/tests.rs` (ADD ~180 lines of tests)
- `crates/prb-grpc/tests/h2_edge_tests.rs` (NEW - ~50 lines if needed)

Total: ~230 new test lines

## Test Plan

1. Generate baseline HTML coverage:
   ```bash
   cargo llvm-cov -p prb-grpc --html
   open target/llvm-cov/html/prb_grpc/index.html
   ```
2. Identify specific uncovered lines in h2.rs (lines 141-256, 258-406, 444-528)
3. Add frame type tests to src/tests.rs
4. Verify coverage increases:
   ```bash
   cargo llvm-cov -p prb-grpc --summary-only
   ```
5. Add HPACK and decoder edge case tests
6. Verify final coverage ≥90%
7. Run full test suite:
   ```bash
   cargo test -p prb-grpc
   ```
8. Commit: "test: Increase prb-grpc coverage to 90% (H2 frame parsing + HPACK edge cases)"

## Blocked By

None - can start immediately

## Blocks

Segment 08 (prb-cli coverage) - CLI gRPC command tests depend on solid grpc crate

## Success Metrics

- prb-grpc coverage: 72.69% → 90%+
- h2.rs coverage: 46.83% → 85%+
- decoder.rs coverage: 81.36% → 88%+
- All 25 existing tests pass
- ~15-20 new test functions added
- No test flakiness

## Notes

- h2.rs has basic frame parser, not full HTTP/2 implementation (simplified HPACK)
- Focus on frame types used in gRPC: DATA, HEADERS, SETTINGS, RST_STREAM (not PUSH_PROMISE, PRIORITY)
- HPACK degradation is expected for mid-stream captures (tested via warning events)
- Static table lookup (lines 444-528) is straightforward const matching
- Real-world HTTP/2 fixture exists at `tests/fixtures/captures/http2/http2-h2c.pcap`
