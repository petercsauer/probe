---
segment: 4
title: "Wire-format Protobuf Decode"
depends_on: [1]
risk: 3/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "feat(decode): add wire-format protobuf decode with heuristic disambiguation"
---

# Segment 4: Wire-format Protobuf Decode

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Implement best-effort protobuf wire-format decoding without schemas, with documented limitations and multi-interpretation output for ambiguous wire types.

**Depends on:** Segment 1 (MCAP storage for CLI integration). Independent of Segments 2 and 3.

## Context: Issues Addressed

### S2-3: protobuf-decode Crate Reference Incorrect

**Core Problem:** The parent plan's Issue 6 references `protobuf-decode (crates.io)` as an existing crate for heuristic protobuf decoding. This crate does not exist on crates.io. The builder would waste cycles searching for a nonexistent dependency.

**Proposed Fix:** Do NOT use `protobuf-decode`. Build a custom wire-format decoder (~200-350 lines). The wire format is simple enough that a dependency is not justified. Actual alternatives on crates.io: `protobin` (v0.6.0, wire-format primitives but no heuristic disambiguation), `decode_raw` (v0.2.0, unmaintained since 2022), `anybuf` (v1.0.0, no fixed-length types). If the custom implementation proves problematic, protobin's MsgDecoder can serve as a fallback parsing foundation.

**Pre-Mortem:** None specific to this segment -- the correction prevents wasted cycles.

### S2-4: Wire-Format Ambiguities Broader Than Stated

**Core Problem:** Wire type 2 ambiguity is correct but incomplete. The protobuf wire format has disambiguation problems across all data-carrying wire types:
- Wire type 0 (VARINT): int32, int64, uint32, uint64, sint32, sint64, bool, enum all use varints. Cannot distinguish signed from unsigned, or bool from integer.
- Wire type 1 (I64): fixed64, sfixed64, double all use 8-byte little-endian. Same bits, three interpretations.
- Wire type 5 (I32): fixed32, sfixed32, float all use 4-byte little-endian. Same bits, three interpretations.

**Proposed Fix:** Handle all ambiguities:
- Wire type 0: Display unsigned value, signed ZigZag-decoded value, bool interpretation (if 0 or 1).
- Wire type 1: Display u64, i64 (signed reinterpretation), f64 (IEEE 754). Suppress f64 if NaN or subnormal.
- Wire type 2: Heuristic cascade: try sub-message -> try UTF-8 string -> hex dump. Recursion depth limit 64.
- Wire type 5: Display u32, i32, f32. Suppress f32 if NaN or subnormal.

Output format: primary interpretation prominently, alternatives in parentheses. Example: `field 1: 150 (varint; also: sint=-75, bool=N/A)`.

**Pre-Mortem:** Multi-interpretation output may confuse users unfamiliar with protobuf wire format. Provide clear documentation and consider `--raw` flag. f64/f32 display of integer values produces nonsensical numbers; only show float if value is plausible (not NaN, not denormalized).

### Parent Issue 6: Schema-less Protobuf Decode (wire-format path)

**Core Problem:** Schema-less decode is inherently best-effort. Wire type 2 is used for strings, bytes, nested messages, packed repeated fields -- indistinguishable without schema.

**Proposed Fix:** Rename to "wire-format decode". Parse raw wire format to extract field numbers, wire types, raw values. For wire type 2, apply heuristic cascade. Always display field numbers, never field names. Output must clearly indicate "WIRE FORMAT DECODE (best-effort, no schema)" at the top. Build custom decoder ~200-350 lines.

**Pre-Mortem:** Heuristic cascade may misidentify byte array as sub-message. Recursive parsing on random binary could produce false positives or infinite recursion. Use recursion depth limit (64) and "at least one valid tag" check.

## Scope

- New module in `crates/decode/` (if Segment 3 has created the crate) or new crate `crates/wire-decode/`
- Modified: `crates/cli/` (add --wire-format flag to inspect)

## Key Files and Context

Protobuf wire format (per developers.google.com/protocol-buffers/docs/encoding):
- Tag = (field_number << 3) | wire_type
- Wire type 0 (VARINT): variable-length integer. Used for int32, int64, uint32, uint64, sint32 (ZigZag), sint64 (ZigZag), bool, enum.
- Wire type 1 (I64): fixed 8 bytes. Used for fixed64, sfixed64, double.
- Wire type 2 (LEN): varint length + N bytes. Used for string, bytes, nested messages, packed repeated fields.
- Wire type 3 (SGROUP): deprecated group start marker.
- Wire type 4 (EGROUP): deprecated group end marker.
- Wire type 5 (I32): fixed 4 bytes. Used for fixed32, sfixed32, float.

Heuristic cascade for wire type 2 (ordered by specificity):
1. Try parsing as nested protobuf message (recursive). Accept if: at least one valid tag parsed, no trailing garbage, all wire types valid.
2. Try UTF-8 decode. Accept if: valid UTF-8, contains mostly printable characters (>80%).
3. Fall back to hex dump.

Recursion depth limit: 64.

Output format example:
```
field 1: 150 (varint; also: sint=-75, bool=N/A)
field 2: {
  field 1: 42 (varint)
} (submessage; 1 field)
field 3: "hello world" (string; 11 bytes)
field 4: 0xdeadbeef... (bytes; 128 bytes)
```

## Implementation Approach

1. Implement in `crates/decode/src/wire_format.rs` (~250-350 lines):
   - `pub fn decode_wire_format(bytes: &[u8]) -> Result<WireMessage, WireDecodeError>`
   - `WireMessage` contains a Vec of `WireField` structs.
   - `WireField` has field_number, wire_type, and `WireValue` enum.
   - `WireValue` variants: Varint(u64), Fixed64([u8; 8]), Fixed32([u8; 4]), LengthDelimited(WireLenValue).
   - `WireLenValue` enum: SubMessage(WireMessage), String(String), Bytes(Vec<u8>).
2. Implement varint decoder (standard LEB128).
3. Implement tag parser: extract field_number and wire_type from varint.
4. Implement heuristic cascade for LEN fields with recursion depth tracking.
5. Implement Display for WireMessage with multi-interpretation annotations.
6. Add `--wire-format` flag to `prb inspect` that shows wire-format decode instead of schema-backed decode.
7. Wire-format output must clearly state "WIRE FORMAT DECODE (best-effort, no schema)" at the top.

## Alternatives Ruled Out

- Use protobin crate for wire parsing. Evaluated: protobin provides MsgDecoder with zero-copy iteration, but wire format parsing is ~50 lines. Adding a dependency is not justified. protobin is a known fallback if implementation proves problematic.
- Use decode_raw crate. Rejected: unmaintained since 2022 (v0.2.0), depends on protofish.
- Use anybuf crate. Rejected: does not support fixed-length types (I32, I64) or packed repeated fields.

## Pre-Mortem Risks

- Heuristic cascade on random binary data may produce deeply nested false-positive sub-messages. Recursion depth limit (64) and "at least one valid tag" check mitigate; consider adding "confidence" indicator.
- UTF-8 heuristic may misidentify binary data that happens to be valid UTF-8. Printable-character ratio check (>80%) helps; consider fallback display showing both interpretations.
- Infinite loops on malformed varints (MSB always set). Add maximum varint length check (10 bytes per protobuf spec).

## Build and Test Commands

- Build: `cargo build -p prb-decode`
- Test (targeted): `cargo nextest run -p prb-decode`
- Test (regression): `cargo nextest run -p prb-core -p prb-storage -p prb-cli`
- Test (full gate): `cargo nextest run --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_wire_varint`: encode field 1 as varint 150, decode, verify field_number=1 and value=150.
   - `test_wire_string`: encode field 2 as string "hello", decode, verify detected as string.
   - `test_wire_nested_message`: encode a nested message, decode, verify sub-message parsed recursively.
   - `test_wire_bytes_fallback`: encode field 3 as random binary bytes, decode, verify falls back to hex.
   - `test_wire_fixed32_float`: encode field 4 as fixed32, decode, verify both u32 and f32 interpretations shown.
   - `test_wire_fixed64_double`: encode field 5 as fixed64, decode, verify both u64 and f64 interpretations shown.
   - `test_wire_recursion_limit`: create deeply nested (100 levels) message bytes, verify decode stops at depth 64 without panic.
   - `test_wire_malformed_varint`: create bytes with never-ending varint (all MSB set), verify error (not hang).
   - `test_wire_empty_input`: decode empty bytes, verify empty WireMessage (not error).
   - `test_wire_zigzag`: encode sint32 field, decode, verify both unsigned and ZigZag-decoded values shown.
   - `test_cli_inspect_wire_format`: run `prb inspect session.mcap --wire-format`, verify wire-format output for protobuf payloads.
2. **Regression tests:** All prior segment and subsection tests pass.
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo nextest run --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changed files are in `crates/decode/` (wire_format module), `crates/cli/`, and `Cargo.toml`.
