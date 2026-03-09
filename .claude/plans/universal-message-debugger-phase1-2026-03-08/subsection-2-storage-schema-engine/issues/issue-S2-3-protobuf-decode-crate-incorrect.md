---
id: "S2-3"
title: "protobuf-decode Crate Reference Incorrect"
risk: 1/10
addressed_by_segments: [4]
---

# Issue S2-3: protobuf-decode Crate Reference Incorrect

**Core Problem:**
The parent plan's Issue 6 references `protobuf-decode (crates.io)` as an existing crate for heuristic protobuf decoding. This crate does not exist on crates.io. The builder would waste cycles searching for a nonexistent dependency.

**Root Cause:**
Library name was likely confused with `protoc --decode_raw` (the protoc subcommand for schema-less decode) or hallucinated during planning.

**Proposed Fix:**
Correct the parent plan's Issue 6 to reference the actual alternatives found on crates.io:

- `protobin` (v0.6.0, crates.io, actively maintained, last release 2026-02-07) -- zero-copy wire-format primitives with MsgDecoder, supports all wire types, no dependencies. Provides the parsing layer but not heuristic disambiguation.
- `decode_raw` (v0.2.0, crates.io, unmaintained since 2022) -- heuristic schema-less decoding built on protofish. Shows the pattern but is not suitable as a dependency.
- `anybuf` (v1.0.0, crates.io, 2025-06-01) -- schema-less Bufany decoder, but does not support fixed-length types or packed encoding.

Recommendation unchanged: build a custom wire-format decoder (~200-350 lines). The wire format is simple enough that a dependency is not justified, and existing crates either lack heuristic logic (protobin) or are unmaintained (decode_raw).

However, `protobin` is worth noting as a fallback: if the custom implementation proves problematic, protobin's MsgDecoder can serve as the parsing foundation.

**Existing Solutions Evaluated:**
See above -- protobin, decode_raw, anybuf all evaluated. None fully solve the problem.

**Alternatives Considered:**

- Adopt protobin and build heuristics on top. Evaluated but not recommended: protobin provides ~50 lines worth of parsing logic. A custom implementation avoids the dependency and gives full control over error recovery.

**Pre-Mortem -- What Could Go Wrong:**

- A builder agent reading the uncorrected parent plan would search for `protobuf-decode` on crates.io, not find it, and waste cycles. This correction prevents that.

**Risk Factor:** 1/10

**Evidence for Optimality:**

- Existing solutions: Direct verification on crates.io confirms no crate named `protobuf-decode` exists. `protobin` is the closest active alternative.
- External evidence: `protoc --decode_raw` is the canonical tool for this task, confirming the custom-build approach.

**Blast Radius:**

- Direct changes: parent plan Issue 6 text correction
- Potential ripple: none (no code yet)
