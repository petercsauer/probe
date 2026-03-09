---
segment: 2
title: "ZMTP Decoder"
depends_on: [1]
risk: 4/10
complexity: Medium
cycle_budget: 15
status: pending
commit_message: "feat(protocol-zmtp): add custom ZMTP wire protocol decoder"
---

# Segment 2: ZMTP Decoder

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Implement a custom ZMTP wire protocol parser that extracts ZeroMQ messages from reassembled TCP streams, including greeting/handshake parsing, multipart message reassembly, metadata extraction, and mid-stream graceful degradation.

**Depends on:** Segment 1 (protocol dispatch infrastructure must exist)

## Context: Issues Addressed

**D1 (dead zmtp crate):** The `zmtp` crate (crates.io) was last updated 2016-06-19, depends on ancient byteorder 0.5.3, and is effectively abandoned. Build a custom ZMTP wire protocol parser. ZMTP 3.0/3.1 wire format: 64-byte greeting (signature, version, mechanism, as-server, filler), handshake commands (READY/ERROR with metadata), traffic frames (1-byte flags + 1-or-8-byte size + body). Implement `ZmtpParser` with `feed(&mut self, data: &[u8]) -> Vec<ZmtpEvent>`. Estimated ~300 lines. Pre-mortem: edge cases (ZMTP 2.0 fallback, CURVE framing); malformed frames; multipart reassembly across TCP boundaries.

**D5 (ZMTP mid-stream limitation):** ZMTP greeting (64 bytes) establishes parsing context. Mid-stream captures lack it. Two-tier strategy: (1) Full greeting detection: bytes 0 and 9 must be 0xFF and 0x7F. (2) Heuristic fallback: valid flag bytes have bits 7-3 zero (8 valid values 0x00-0x07); scan for these, validate plausible frame boundaries, log warning. (3) Give up gracefully: emit raw TCP with diagnostic. Pre-mortem: heuristic false positives on binary data; CURVE encryption makes bodies opaque; performance overhead.

## Scope

- Custom ZMTP 3.0/3.1 wire protocol parser (~300 lines)
- Greeting parsing (64-byte fixed format: signature, version, mechanism, as-server)
- NULL security handshake (READY command with metadata properties)
- Traffic frame parsing (flags byte + size + body)
- Multipart message reassembly (MORE flag handling)
- Metadata extraction: socket type, identity, mechanism from READY command
- Mid-stream heuristic detection and graceful degradation
- Correlation metadata: connection_id, socket_type, identity, topic_prefix for PUB/SUB
- CLI integration: `prb inspect` showing ZMTP message details

## Key Files and Context

ZMTP 3.0 wire format (RFC 23/ZMTP):
- Greeting (64 bytes total): `signature(0xFF + 8 padding bytes + 0x7F) + version(major=0x03, minor=0x00|0x01) + mechanism(20 bytes, null-padded ASCII, e.g. "NULL") + as-server(0x00|0x01) + filler(31 zero bytes)`.
- Commands: `flag_byte(0x04 for short command, 0x06 for long command) + size(1 byte for short, 8 bytes for long) + body`. Body starts with `command_name_length(1 byte) + command_name + command_data`.
- Message frames: `flag_byte + size + body`. Flag bits: bit 0 = MORE (more frames follow in this message), bit 1 = LONG (8-byte size instead of 1-byte), bit 2 = COMMAND (this is a command, not a message). Bits 7-3 MUST be zero per spec.
- READY command metadata: list of `(name_length(1 byte) + name + value_length(4 bytes, network order) + value)` properties. Standard properties include `Socket-Type` and `Identity`.

The protocol dispatch from Segment 1 routes streams to this decoder. ZMTP can be identified by magic bytes (`0xFF` at byte 0, `0x7F` at byte 9 of the greeting) or by port hint from user.

For PUB/SUB sockets, the first frame of a message is the subscription topic. This is the primary correlation key. For REQ/REP sockets, messages alternate request/response. Correlation uses the socket identity if available.

Only the NULL security mechanism is in scope for Phase 1. PLAIN and CURVE require additional handshake parsing that can be added in a future phase.

Error handling convention: library crates use `thiserror`. The `no-ignore-failure` workspace rule requires that parsing errors are surfaced, not silently swallowed.

## Implementation Approach

1. Create a `ZmtpParser` struct with states: `AwaitingGreeting`, `AwaitingHandshake`, `Traffic`, `Degraded`.
2. Implement `feed(&mut self, data: &[u8]) -> Result<Vec<ZmtpEvent>>` where `ZmtpEvent` includes `Greeting { version, mechanism, as_server }`, `Ready { metadata: HashMap<String, Vec<u8>> }`, `Message { frames: Vec<Vec<u8>> }`, `Command { name: String, data: Vec<u8> }`.
3. Greeting detection: Check bytes 0 and 9 for ZMTP signature (`0xFF` and `0x7F`). If match, parse full 64-byte greeting. If no match and stream is from a known ZMQ port, attempt heuristic frame detection.
4. Handshake: After greeting, parse READY command. Extract `Socket-Type` and `Identity` properties from metadata.
5. Traffic: Parse frames using flag byte. Accumulate multipart messages (frames with MORE=1) until final frame (MORE=0). Emit complete message.
6. For PUB/SUB: extract topic from first frame of each message (by convention, the first frame is the topic prefix).
7. Emit `DebugEvent` for each complete message with metadata (socket type, identity, topic if PUB/SUB).
8. Mid-stream fallback: If greeting not detected, scan for valid flag bytes (bits 7-3 must be zero) followed by plausible sizes. Parse frames heuristically. Set degraded mode flag on all emitted events. Log warning per `no-ignore-failure` convention.

## Alternatives Ruled Out

- Using the `zmtp` crate (v0.6.0, 2016). Dead, ancient dependencies.
- Extracting parser from rzmq. Tightly coupled to async runtime, MPL-2.0 license complexity.
- Supporting PLAIN/CURVE security in Phase 1. Excessive scope; NULL is sufficient for initial debugging use cases. PLAIN/CURVE can be added incrementally later.

## Pre-Mortem Risks

- ZMTP version negotiation edge cases (ZMTP 2.0 fallback) may produce confusing parser states. Support only ZMTP 3.0/3.1 and emit clear errors for older versions.
- Multipart messages with very large frame counts could consume excessive memory. Add a configurable frame count limit with a sensible default.
- Heuristic mid-stream detection may produce false positives on binary TCP data that resembles ZMTP frames.

## Build and Test Commands

- Build: `cargo build -p prb-protocol-zmtp`
- Test (targeted): `cargo test -p prb-protocol-zmtp`
- Test (regression): `cargo test -p prb-core -p prb-protocol-grpc`
- Test (full gate): `cargo test --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_zmtp_greeting_parse`: Feed a valid 64-byte ZMTP 3.0 greeting with NULL mechanism. Verify version, mechanism, and as-server are correctly extracted.
   - `test_zmtp_ready_metadata`: Feed greeting + READY command with Socket-Type=PUB and Identity=test-pub. Verify metadata extraction.
   - `test_zmtp_single_frame_message`: Feed greeting + handshake + a single-frame message (MORE=0). Verify message body is correctly extracted.
   - `test_zmtp_multipart_message`: Feed a 3-frame multipart message (MORE=1, MORE=1, MORE=0). Verify all frames are assembled into one message.
   - `test_zmtp_long_frame`: Feed a frame with LONG=1 flag and 8-byte size field. Verify correct parsing.
   - `test_zmtp_pubsub_topic`: Feed a PUB socket message where first frame is topic "sensor.temp" and second frame is payload. Verify topic extraction.
   - `test_zmtp_mid_stream_degraded`: Feed bytes without a greeting. Verify degraded mode is entered with warning and heuristic frame parsing is attempted.
   - `test_zmtp_invalid_version`: Feed a greeting with version 2.0. Verify an appropriate error/warning is emitted (not silently ignored).
2. **Regression tests:** All tests from Segment 1 and Subsections 1-3 continue passing.
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo test --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changed files are in the ZMTP decoder crate and CLI integration. Out-of-scope changes documented.
