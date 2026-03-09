---
segment: 3
title: "Schema-backed Protobuf Decode"
depends_on: [2]
risk: 3/10
complexity: Low
cycle_budget: 10
status: pending
commit_message: "feat(decode): add schema-backed protobuf decode with prost-reflect"
---

# Segment 3: Schema-backed Protobuf Decode

> This file is a self-contained handoff contract for an iterative-builder subagent.

**Goal:** Implement protobuf message decoding using loaded schemas, producing structured human-readable output from raw protobuf bytes.

**Depends on:** Segment 2 (SchemaRegistry provides MessageDescriptor for decode)

## Context: Issues Addressed

### Parent Issue 6: Schema-less Protobuf Decode Oversold (schema-backed path)

**Core Problem:** The plan lists "Schema-less decode" as co-equal with schema-backed decode. Protobuf wire format has fundamental ambiguity: wire type 2 (length-delimited) is used for strings, bytes, nested messages, and packed repeated fields. Without a schema, these are indistinguishable. The schema-backed path is the "properly functioning" mode that produces authoritative decode with field names and types.

**Proposed Fix (schema-backed path):** Use prost-reflect's `DynamicMessage::decode(descriptor, buf)` when a schema is available. This produces structured output with correct field names, types, and nested message handling. The schema-backed path is the primary decode path when schemas are loaded; wire-format decode (Segment 4) is the fallback for schema-less inspection.

**Pre-Mortem:** DynamicMessage::decode may produce confusing errors for truncated or partially valid payloads. Wrap errors with context (payload size, expected message type). Users may mistake wire-format output for authoritative decode; schema-backed output is authoritative when schemas match.

## Scope

- New crate: `crates/decode/` (prb-decode) -- or new module in existing crate
- Modified: `crates/cli/` (prb inspect shows decoded payloads)

## Key Files and Context

Segment 2 produces:
- `crates/schema/src/registry.rs` -- SchemaRegistry with `get_message(fqn) -> Option<MessageDescriptor>`
- MessageDescriptor is from prost-reflect and supports `DynamicMessage::decode(descriptor, bytes)`

prost-reflect DynamicMessage API:
- `DynamicMessage::decode(descriptor: MessageDescriptor, buf: impl Buf) -> Result<Self, DecodeError>`
- DynamicMessage implements Display for human-readable output
- DynamicMessage implements serde::Serialize for JSON output
- Fields accessed via `message.get_field_by_name("field")` returning a `Value` enum
- Nested messages, repeated fields, maps, oneofs all supported

DebugEvent has a payload field (raw bytes) and a type_name field (FQN of the protobuf message).

## Implementation Approach

1. Create `crates/decode/` crate with deps: `prost-reflect`, plus workspace deps (prb-core, thiserror, tracing, bytes).
2. Implement schema-backed decode function:
   ```rust
   pub fn decode_with_schema(
       payload: &[u8],
       descriptor: &MessageDescriptor,
   ) -> Result<DecodedMessage, DecodeError>
   ```
   Where `DecodedMessage` wraps prost-reflect's DynamicMessage with display formatting.
3. Implement `DecodedMessage` display:
   - Tree-formatted output showing field names, types, and values
   - Nested messages indented
   - Repeated fields shown as lists
   - Bytes fields shown as hex with length
   - Enum fields shown as name (numeric value)
4. Integrate with `prb inspect`:
   - When inspecting events with protobuf payloads, attempt schema-backed decode if schemas are available.
   - If decode succeeds, show decoded fields instead of raw hex.
   - If decode fails (wrong schema, corrupted payload), show error and fall back to raw display.
   - Add `--decode-type <fqn>` flag to force a specific message type for decode.
5. Add JSON output mode: `prb inspect --format json` outputs DynamicMessage as JSON (via serde).

## Alternatives Ruled Out

- Use prost codegen (compile-time generated structs). Rejected: requires build-time protobuf compilation for user schemas; runtime decode via prost-reflect is the whole point.
- Use the protobuf (rust-protobuf) crate's reflect module. Rejected: ecosystem mismatch with prost.

## Pre-Mortem Risks

- DynamicMessage::decode may produce confusing errors for truncated or partially valid payloads. Wrap errors with context (payload size, expected message type).
- Performance of DynamicMessage decode is slower than compiled prost. For the inspect use case (human reads output), this is acceptable.
- prost-reflect's Display impl may not format output the way we want. If so, implement custom display logic.

## Build and Test Commands

- Build: `cargo build -p prb-decode`
- Test (targeted): `cargo nextest run -p prb-decode`
- Test (regression): `cargo nextest run -p prb-core -p prb-schema -p prb-storage -p prb-cli`
- Test (full gate): `cargo nextest run --workspace`

## Exit Criteria

1. **Targeted tests:**
   - `test_decode_simple_message`: encode a known message with prost, decode with DynamicMessage, verify all fields match.
   - `test_decode_nested_message`: message with nested sub-messages, verify recursive decode.
   - `test_decode_repeated_fields`: message with repeated int32 and repeated string, verify list output.
   - `test_decode_enum_field`: message with enum field, verify name and numeric value shown.
   - `test_decode_wrong_schema`: attempt decode with mismatched schema, verify error (not panic).
   - `test_decode_truncated_payload`: decode truncated bytes, verify error with context.
   - `test_decode_json_output`: decode message, serialize to JSON, verify valid JSON with correct field names.
   - `test_cli_inspect_decoded`: ingest fixture with protobuf payload and schema, run `prb inspect`, verify decoded fields in output.
2. **Regression tests:** All prior segment and subsection tests pass.
3. **Full build gate:** `cargo build --workspace`
4. **Full test gate:** `cargo nextest run --workspace`
5. **Self-review gate:** No dead code, no commented-out blocks, no TODO hacks, no changes outside stated scope.
6. **Scope verification gate:** Changed files are in `crates/decode/`, `crates/cli/`, and `Cargo.toml`.
