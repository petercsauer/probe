---
id: "D5"
title: "ZMTP Mid-Stream Capture Limitation"
risk: 4/10
addressed_by_segments: [2]
---
# Issue D5: ZMTP Mid-Stream Capture Limitation

**Core Problem:**
ZMTP connections begin with a fixed 64-byte greeting that negotiates version and security mechanism. If a PCAP capture starts after the greeting has been exchanged, the parser cannot determine frame boundaries, security mode, or connection metadata. This is the same class of problem as HPACK statefulness (Issue 2 in the parent plan).

**Root Cause:**
ZMTP is a stateful protocol where the greeting establishes parsing context for all subsequent traffic.

**Proposed Fix:**
Implement a two-tier detection strategy:
1. **Full greeting detection:** If the first bytes of a reassembled TCP stream match ZMTP greeting signature (`0xFF` at byte 0, `0x7F` at byte 9), proceed with full protocol parsing.
2. **Heuristic fallback:** If no greeting is detected, attempt to parse frames using heuristic flag-byte detection. Valid ZMTP frame flags have bits 7-3 as zero, giving only 8 valid flag values (0x00-0x07). Scan for these, validate that the subsequent size field produces a plausible frame boundary. Log a warning that parsing is best-effort.
3. **Give up gracefully:** If heuristics fail, emit the stream as raw TCP data with a diagnostic message.

**Existing Solutions Evaluated:**
- N/A -- no tool handles mid-stream ZMTP recovery. Wireshark's ZMTP dissector also fails on mid-stream captures.

**Alternatives Considered:**
- Require full-connection captures only. Rejected: same reasoning as the HPACK issue (too restrictive for real-world use).

**Pre-Mortem -- What Could Go Wrong:**
- Heuristic frame detection produces false positives on binary data that happens to have valid-looking flag bytes.
- ZMTP with CURVE encryption makes frame bodies opaque; heuristic parsing on encrypted streams will fail.
- Performance overhead of scanning for frame boundaries in large captures.

**Risk Factor:** 4/10

**Evidence for Optimality:**
- External evidence: Wireshark's ZMTP dissector also requires full connection observation, confirming this is a fundamental protocol limitation.
- External evidence: The ZMTP spec's flag byte constraints (bits 7-3 must be zero) provide a useful heuristic signal not available in most protocols.

**Blast Radius:**
- Direct: ZMTP decoder (detection/parsing logic)
- Ripple: CLI output (degraded-mode warnings)
