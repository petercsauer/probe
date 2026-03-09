---
id: "D1"
title: "zmtp Crate Is Dead"
risk: 4/10
addressed_by_segments: [2]
---
# Issue D1: zmtp Crate Is Dead

**Core Problem:**
The parent plan lists `zmtp` (crates.io) as a key dependency for the ZMQ/ZMTP decoder, but the crate was last updated on 2016-06-19, depends on ancient `byteorder` 0.5.3, has 52 downloads in 90 days, and its repository is on Bitbucket. It is effectively abandoned.

**Root Cause:**
The parent plan selected the crate by name match without checking maintenance status.

**Proposed Fix:**
Build a custom ZMTP wire protocol parser. The ZMTP 3.0/3.1 wire format is well-specified (RFC 23/ZMTP, RFC 37/ZMTP) and consists of:
- 64-byte greeting (fixed format: signature, version, mechanism, as-server, filler)
- Handshake commands (READY/ERROR with metadata properties)
- Traffic frames: 1-byte flags (MORE|LONG|COMMAND bits) + 1-or-8-byte size + body

Implementation: a `ZmtpParser` struct maintaining greeting/handshake state, with a `feed(&mut self, data: &[u8]) -> Vec<ZmtpEvent>` method that emits greeting, command, and message events. Estimated ~300 lines.

**Existing Solutions Evaluated:**
- `zmtp` v0.6.0 (crates.io, 3,925 total downloads, Bitbucket repo) -- Last updated 2016-06-19. Depends on `byteorder` 0.5.3 (ancient). **Dead. Rejected.**
- `rzmq` v0.5.13 (crates.io, 3,004 downloads, updated 2026-02-02, MPL-2.0) -- Active, has internal `ZmtpCodec` and `ZmtpManualParser`. But tightly coupled to tokio async runtime; not extractable for passive offline parsing. Useful as reference implementation. **Rejected for direct use.**
- `zeromq/zmq.rs` (GitHub, 1.1K stars) -- Has `zmq_codec.rs` internally. Also async-coupled (`asynchronous_codec` traits). Not extractable. **Rejected.**
- `zedmq` (GitHub) -- Minimal, also live-connection-oriented. **Rejected.**

**Alternatives Considered:**
- Fork and modernize the `zmtp` crate. Rejected: the crate is small enough that starting fresh is faster than updating 10-year-old code with ancient dependencies.
- Use `rzmq` as a dependency and extract its parser. Rejected: MPL-2.0 license adds complexity, and the parser is deeply integrated with the async engine.

**Pre-Mortem -- What Could Go Wrong:**
- Custom parser may miss ZMTP edge cases (version negotiation fallback to ZMTP 2.0/1.0, CURVE security mechanism framing).
- Parser may not handle malformed frames gracefully (captures often contain truncated packets).
- Multipart message reassembly across TCP segment boundaries is tricky.

**Risk Factor:** 4/10

**Evidence for Optimality:**
- External evidence: ZMTP spec (RFC 23) is simple enough that Wireshark's Lua-based dissector (`whitequark/zmtp-wireshark`) is ~400 lines. A Rust parser should be comparable.
- Existing solutions: Reference implementations in rzmq and zmq.rs provide behavioral specifications and implicit test vectors.

**Blast Radius:**
- Direct: new ZMTP decoder crate
- Ripple: Cargo.toml dependency list (remove `zmtp`, add nothing -- it is custom code)
