---
id: "3"
title: "Wrong HTTP/2 Library"
risk: 3/10
addressed_by_subsections: [4]
---

# Issue 3: Wrong HTTP/2 Library for Offline Parsing

**Core Problem:**
The plan specifies the `h2` crate for HTTP/2 frame parsing in the gRPC adapter. `h2` is an async client/server implementation that expects to participate in a live connection. It cannot passively parse captured frames from a byte buffer.

**Root Cause:**
Library selection was based on name recognition ("h2 = HTTP/2") without evaluating whether the crate's API supports passive/offline parsing.

**Proposed Fix:**
Replace `h2` with `h2-sans-io` (synchronous, sans-I/O HTTP/2 frame codec) for the gRPC protocol adapter. `h2-sans-io` accepts raw bytes, parses frames, handles CONTINUATION assembly, and decompresses HPACK headers -- all without requiring an async runtime or active connection.

**Existing Solutions Evaluated:**
- `h2` (crates.io, 65M+ downloads) -- full async HTTP/2 client/server. Cannot parse offline captures. Rejected.
- `h2-sans-io` (crates.io) -- synchronous, no-I/O HTTP/2 codec with HPACK. Purpose-built for this use case. Adopted.
- `fluke-h2-parse` (crates.io) -- nom-based frame parser. Lighter than h2-sans-io but lacks HPACK decompression. Could be used as a fallback if h2-sans-io proves too heavy.

**Recommendation:** Adopt `h2-sans-io` as primary. Keep `fluke-h2-parse` as a noted fallback.

**Alternatives Considered:**
- Write a custom HTTP/2 frame parser. Rejected: HTTP/2 framing is well-specified but has many edge cases (CONTINUATION, padding, priority); existing libraries handle these correctly.

**Pre-Mortem -- What Could Go Wrong:**
- `h2-sans-io` is newer and less battle-tested than `h2`. May have edge-case bugs with unusual frame sequences.
- API may not expose enough control for our use case (e.g., handling malformed frames gracefully).

**Risk Factor:** 3/10

**Evidence for Optimality:**
- Existing solutions: `h2-sans-io` is explicitly designed for the sans-I/O pattern needed by offline analysis tools and WASM environments.
- External evidence: The Rust ecosystem consensus (docs.rs documentation, crate descriptions) is that `h2` is for active connections and sans-I/O variants are for passive parsing.

**Blast Radius:**
- Direct: gRPC protocol adapter dependency list
- Ripple: none (swap is contained to one crate's Cargo.toml and import paths)
