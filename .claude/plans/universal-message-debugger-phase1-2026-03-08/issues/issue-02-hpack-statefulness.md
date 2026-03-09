---
id: "2"
title: "HPACK Statefulness Breaks Mid-Stream"
risk: 5/10
addressed_by_subsections: [4]
---

# Issue 2: HPACK Statefulness Breaks Mid-Stream Capture

**Core Problem:**
HTTP/2 uses HPACK header compression with a stateful dynamic table built incrementally from connection start. If a PCAP starts mid-stream, the tool cannot decode HTTP/2 headers because the dynamic table context is missing. The original plan does not mention HPACK at all.

**Root Cause:**
The plan treats HTTP/2 frame parsing as stateless, but HPACK is inherently stateful and requires observing the full connection from the initial SETTINGS frame.

**Proposed Fix:**
Document the requirement for full-connection captures in gRPC mode. Implement graceful degradation: when HPACK context is missing, log a warning and fall back to payload-only analysis (protobuf bodies are not HPACK-compressed, only headers are). Add a `--hpack-tolerant` flag that substitutes raw header bytes when decompression fails instead of aborting.

**Existing Solutions Evaluated:**
- `hpack` crate (crates.io, ~100K downloads) -- HPACK encoder/decoder. Can be used for header decompression when context is available.
- `h2-sans-io` (crates.io) -- includes HPACK decompression support as part of its HTTP/2 frame codec.
- `fluke-h2-parse` (crates.io) -- nom-based HTTP/2 frame parser. Does not handle HPACK; only parses frame structure.

**Recommendation:** Use `h2-sans-io` which bundles HPACK decompression with frame parsing. Fall back to raw bytes when decompression fails.

**Alternatives Considered:**
- Require full-connection captures only. Rejected: too restrictive for real-world use where captures often start after connections are established.
- Reconstruct HPACK state heuristically. Rejected: impossible without the initial dynamic table entries.

**Pre-Mortem -- What Could Go Wrong:**
- Users expect header decoding to always work and file bugs when it doesn't.
- Graceful degradation might hide real parsing bugs (masking errors as "missing HPACK context").
- Some gRPC metadata (method names, authority) is in headers; losing it degrades correlation quality.

**Risk Factor:** 5/10

**Evidence for Optimality:**
- External evidence: Wireshark has the same limitation and documents it explicitly (HTTP/2 dissector docs note that mid-stream captures produce "HPACK - Could Not Decode" warnings).
- Existing solutions: `h2-sans-io` provides integrated HPACK support, avoiding the need to wire up a separate HPACK library.

**Blast Radius:**
- Direct: gRPC protocol adapter
- Ripple: correlation engine (may lack method names for mid-stream captures)
