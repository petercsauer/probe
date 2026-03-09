---
id: "6"
title: "Schema-less Protobuf Decode Oversold"
risk: 3/10
addressed_by_subsections: [2]
---

# Issue 6: Schema-less Protobuf Decode Oversold

**Core Problem:**
The plan lists "Schema-less decode" as a co-equal mode alongside schema-backed decode. Protobuf wire format has fundamental ambiguity: wire type 2 (length-delimited) is used for strings, bytes, nested messages, and packed repeated fields. Without a schema, these are indistinguishable.

**Root Cause:**
The plan does not account for the information-theoretic limitations of the protobuf wire format.

**Proposed Fix:**
Rename to "wire-format decode" and document its limitations explicitly. Implementation: parse raw wire format to extract field numbers, wire types, and raw values. For wire type 2, apply heuristic cascade: (1) try recursive sub-message parse, (2) try UTF-8 string decode, (3) fall back to hex dump. Always display field numbers, never field names. Output must clearly indicate this is best-effort.

**Existing Solutions Evaluated:**
- `protobuf-decode` (crates.io) -- attempts heuristic protobuf decoding without schemas. Small crate, limited maintenance.
- `prost-reflect` (crates.io, 0.16.3, actively maintained) -- provides `DynamicMessage` which requires a schema. Does not help for schema-less case.
- Wireshark's "Decode As... Protobuf" without schema -- applies similar heuristics to what we propose.

**Recommendation:** Build a small custom wire-format decoder. The protobuf wire format is simple (5 wire types, varint encoding). A custom implementation of ~200 lines is appropriate. Use heuristics for type 2 disambiguation.

**Alternatives Considered:**
- Remove schema-less mode entirely. Rejected: it's still useful for quick inspection even with limitations.
- Use `protobuf-decode` crate. Rejected: undermaintained; the wire format is simple enough that a custom implementation avoids a fragile dependency.

**Pre-Mortem -- What Could Go Wrong:**
- Heuristic cascade misidentifies a byte array as a sub-message, producing misleading output.
- Users mistake wire-format output for authoritative decode and file bugs about "wrong" field names.
- Recursive sub-message parsing on random binary data could produce false positives or infinite recursion.

**Risk Factor:** 3/10

**Evidence for Optimality:**
- External evidence: protobuf encoding spec (developers.google.com/protocol-buffers/docs/encoding) documents the wire type ambiguity explicitly.
- External evidence: Stack Overflow consensus (multiple high-vote answers) confirms that schema-less protobuf decode is inherently best-effort.

**Blast Radius:**
- Direct: protobuf decode engine
- Ripple: CLI output formatting (must indicate confidence level of schema-less decode)
