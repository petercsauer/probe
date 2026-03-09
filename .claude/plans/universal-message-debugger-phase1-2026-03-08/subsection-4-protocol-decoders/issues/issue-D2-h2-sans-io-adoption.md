---
id: "D2"
title: "h2-sans-io Has Near-Zero Adoption"
risk: 5/10
addressed_by_segments: [1]
---
# Issue D2: h2-sans-io Has Near-Zero Adoption

**Core Problem:**
The parent plan specifies `h2-sans-io` as the HTTP/2 frame codec, but the crate was published 3 weeks ago (2026-02-15), has only 107 total downloads, one version (0.1.0), and a single author. This is a significant bus-factor and quality risk for a foundational dependency.

**Root Cause:**
The crate was selected because it is the only Rust crate that provides synchronous, offline HTTP/2 frame parsing with integrated HPACK. The alternatives (hyperium/h2, fluke) are all async or incomplete.

**Proposed Fix:**
Use h2-sans-io as primary but with explicit mitigations:
1. Pin exact version in Cargo.toml (`h2-sans-io = "=0.1.0"`).
2. Write extensive integration tests against known-good HTTP/2 byte sequences.
3. Document fallback path: `fluke-h2-parse` (v0.1.1, from bearcove/fluke, maintained by fasterthanlime) for frame parsing + `fluke-hpack` (v0.3.1, 70K downloads, recommended by RustSec advisory RUSTSEC-2023-0084) for header decompression. This combination replicates h2-sans-io's functionality with two crates instead of one.
4. Vendor the crate (copy source into workspace) if maintenance concern materializes.

**Existing Solutions Evaluated:**
- `h2-sans-io` v0.1.0 (crates.io, 107 downloads, MIT, created 2026-02-15) -- Correct API (`H2Codec::process(&bytes) -> Vec<H2Event>`), handles CONTINUATION assembly, HPACK via fluke-hpack, flow control. Very new. **Adopted with mitigations.**
- `fluke-h2-parse` v0.1.1 (crates.io, from bearcove/fluke, maintained by fasterthanlime) -- nom-based HTTP/2 frame parser. Parses frame structure but does NOT handle HPACK decompression. More established author. **Documented as fallback in combination with fluke-hpack.**
- `fluke-hpack` v0.3.1 (crates.io, 70K downloads, 40K/90 days, MIT) -- Fork of unmaintained `hpack` crate. Recommended by RustSec advisory. Well-maintained. **Confirmed as solid transitive dependency.**
- `h2` (hyperium, crates.io, 65M+ downloads) -- Async client/server. Cannot parse offline captures. **Rejected.**

**Alternatives Considered:**
- Build a custom HTTP/2 frame parser. Rejected: HTTP/2 has many frame types, CONTINUATION assembly, padding, priority, and flow control. h2-sans-io handles these correctly. Reimplementing is unjustified.
- Use fluke-h2-parse + fluke-hpack from the start (skip h2-sans-io). Considered but h2-sans-io provides a cleaner integrated API. If h2-sans-io fails, this becomes the fallback.

**Pre-Mortem -- What Could Go Wrong:**
- h2-sans-io has a bug in CONTINUATION assembly that corrupts header blocks for large header sets.
- The crate is abandoned and a security issue in fluke-hpack requires an update h2-sans-io does not pick up.
- API changes in a future version break the integration.

**Risk Factor:** 5/10

**Evidence for Optimality:**
- Existing solutions: fluke-hpack (the core dependency) is well-maintained and recommended by RustSec.
- External evidence: The sans-I/O pattern is the correct architecture for offline protocol analysis (sans-io.readthedocs.io).

**Blast Radius:**
- Direct: gRPC decoder module
- Ripple: none if fallback path is documented
