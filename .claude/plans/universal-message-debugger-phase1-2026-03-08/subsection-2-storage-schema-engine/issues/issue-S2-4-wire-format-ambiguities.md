---
id: "S2-4"
title: "Wire-Format Ambiguities Broader Than Stated"
risk: 2/10
addressed_by_segments: [4]
---

# Issue S2-4: Wire-Format Ambiguities Broader Than Stated

**Core Problem:**
The parent plan's Issue 6 states: "wire type 2 (length-delimited) is used for strings, bytes, nested messages, and packed repeated fields. Without a schema, these are indistinguishable." This is correct but incomplete. The protobuf wire format has disambiguation problems across all data-carrying wire types:

- **Wire type 0 (VARINT):** int32, int64, uint32, uint64, sint32, sint64, bool, and enum all use varints. sint32/sint64 use ZigZag encoding; others use raw varints. Without a schema, you cannot distinguish signed from unsigned, or bool from integer.
- **Wire type 1 (I64):** fixed64, sfixed64, and double all use 8-byte little-endian. Same bits, three possible interpretations.
- **Wire type 5 (I32):** fixed32, sfixed32, and float all use 4-byte little-endian. Same bits, three possible interpretations.

**Root Cause:**
The plan focused on the most visible ambiguity (type 2) and overlooked the numeric type ambiguities.

**Proposed Fix:**
The wire-format decoder must handle all ambiguities:

- **Wire type 0:** Display as both unsigned and signed (ZigZag-decoded) values. If value is 0 or 1, also note it could be bool.
- **Wire type 1:** Display as uint64, int64 (signed reinterpretation), and f64 (IEEE 754). Let the user decide which interpretation is correct.
- **Wire type 2:** Heuristic cascade as planned: try sub-message -> try UTF-8 string -> hex dump. Add recursion depth limit of 64.
- **Wire type 5:** Display as uint32, int32 (signed reinterpretation), and f32 (IEEE 754).

Output format should show the "most likely" interpretation prominently with alternatives in parentheses:

```
field 1: 150 (varint; also: sint=-75, bool=N/A)
field 2: "hello world" (string; 11 bytes)
field 3: {nested message} (1 field parsed)
field 4: 3.14 (float; also: fixed32=1078523331)
```

**Existing Solutions Evaluated:**
N/A -- internal design decision about output formatting and heuristic logic.

**Alternatives Considered:**

- Show only one interpretation per wire type (e.g., always unsigned for varints). Rejected: loses information that could help the user identify the correct type.
- Show all interpretations with equal weight. Rejected: too noisy. A primary interpretation with alternatives in parentheses balances completeness and readability.

**Pre-Mortem -- What Could Go Wrong:**

- Multi-interpretation output is confusing for users unfamiliar with protobuf wire format. Mitigation: clear documentation and a `--raw` flag that shows only wire types and hex values.
- f64/f32 display of integer values produces nonsensical floating-point numbers. Mitigation: only show float interpretation if the value is a plausible float (not NaN, not denormalized, not extremely large).

**Risk Factor:** 2/10

**Evidence for Optimality:**

- External evidence: The protobuf encoding specification (developers.google.com/protocol-buffers/docs/encoding) documents all five wire types and their ambiguities.
- External evidence: `protoc --decode_raw` shows only varints and raw bytes, without multi-interpretation display. Our approach is strictly more informative.

**Blast Radius:**

- Direct changes: wire-format decoder output formatting
- Potential ripple: CLI display formatting, documentation
