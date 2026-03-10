---
issue: 2
title: "Mid-stream capture detection failures"
severity: High
segments_affected: [1, 2, 3]
status: open
---

# Issue 2: Mid-Stream Capture Detection Failures

## Problem

Protocol detection relies on inspecting the **first bytes** of a stream. But
PCAP captures often start in the middle of an existing TCP connection — the
initial handshake and protocol greeting happened before capture began. In these
cases:

- **gRPC/HTTP2**: No HTTP/2 connection preface. First bytes are mid-stream
  HTTP/2 frames (DATA, HEADERS). The 9-byte frame header is detectable but
  with lower confidence (0.6 vs 0.95).

- **ZMTP**: No 64-byte greeting. First bytes are ZMTP traffic frames. These
  are generic binary data with no reliable magic bytes.

- **RTPS**: Less affected — each UDP datagram is independent and contains the
  RTPS header. But fragmented DDS messages may lack the header.

## Impact

- Some TCP streams may be misclassified as `RawTcp` when they're actually gRPC
  or ZMQ traffic
- Users who start capture mid-session get degraded decoding
- Decoder state machines may break if they receive data without the expected
  handshake/greeting

## Mitigation Strategies

### M1: Heuristic HTTP/2 frame detection (Segment 1)

The `GrpcDetector` already includes a heuristic path for mid-stream captures:
- Check for valid HTTP/2 frame header (9 bytes: length[3] + type[1] + flags[1]
  + reserved[1] + stream_id[4])
- Valid frame types: 0x00-0x09
- Reasonable length: <16MB
- Confidence: 0.6 (lower than preface detection)

### M2: Port-based boosting (Segment 1)

When the port mapping detector returns a low-confidence hit (e.g., port 50051
→ gRPC at 0.5), AND the heuristic detector also matches (0.6), the registry
can combine evidence. If port + heuristic both suggest the same protocol,
boost confidence above threshold.

### M3: Decoder tolerance (Segment 3)

Decoders must gracefully handle streams that start mid-session:
- `GrpcDecoder`: Already handles HPACK degradation (emits warnings about
  header table state loss). Should accept streams without preface.
- `ZmqDecoder`: Should attempt to detect ZMTP frame boundaries by scanning
  for the 1-byte or 9-byte frame length header pattern. (Existing mid-stream
  heuristic in `prb-zmq`.)
- `DdsDecoder`: Each datagram is independent; not affected.

### M4: User override escape hatch (Segment 3)

The `--protocol` CLI flag allows users to force a protocol when auto-detection
fails: `prb ingest capture.pcap --protocol grpc`. This always works regardless
of stream content.

### M5: Detection learning (future)

After the decoder successfully processes data, update the detection cache. If
a stream initially classified as `RawTcp` is later decoded successfully by a
decoder (e.g., because the user ran with `--protocol`), record that stream's
port/address pattern for future auto-detection. This is a Phase 3 feature.

## Acceptance Criteria

- Mid-stream HTTP/2 frames are detected with confidence ≥ 0.6
- Port + heuristic evidence combines to reach threshold
- Decoders emit warnings (not errors) when greeting/preface is missing
- `--protocol` override always works as a fallback
- Test: capture starting mid-gRPC-session still produces decoded events
  (with warnings about HPACK state)
