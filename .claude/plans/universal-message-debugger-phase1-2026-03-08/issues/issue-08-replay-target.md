---
id: "8"
title: "Replay Target Undefined"
risk: 3/10
addressed_by_subsections: [5]
---

# Issue 8: Replay Target Undefined

**Core Problem:**
Phase 12 says "replay normalized events" but never specifies where they're replayed to. "Replay" means fundamentally different things depending on the target: terminal dump with timing, protocol-faithful re-emission (requires client implementations for every protocol), or piped output for external tools.

**Root Cause:**
The replay feature was specified by analogy ("like replaying a recording") without defining the output interface.

**Proposed Fix:**
Define Phase 1 replay as structured output to stdout with original timing preserved. Events are emitted in chronological order with configurable speed multiplier (1x, 2x, 0.5x, max). Output format matches `prb inspect` output. This is useful for piping to other tools, visual debugging, and building muscle memory before Phase 2 adds protocol-level re-emission.

CLI: `prb replay session.mcap [--speed 2.0] [--filter 'transport=grpc'] [--format json|table]`

**Existing Solutions Evaluated:**
- N/A -- this is an internal design decision about output interface. No external tool solves "replay our custom event model."

**Alternatives Considered:**
- Protocol-faithful re-emission (actually send gRPC calls, ZMQ messages, etc.). Rejected for Phase 1: requires maintaining client implementations for every protocol, authentication handling, endpoint configuration. Suitable for Phase 2+.
- Write replayed events to a new MCAP file (time-filtered copy). Rejected as primary mode: useful but doesn't provide the real-time visual feedback that makes replay valuable.

**Pre-Mortem -- What Could Go Wrong:**
- Timing accuracy depends on tokio timer resolution; sub-millisecond event spacing may not replay accurately.
- High-throughput sessions (100k+ events/sec) may not be replayable in real-time due to stdout buffering.
- Users may expect protocol-level replay and be disappointed by text output.

**Risk Factor:** 3/10

**Evidence for Optimality:**
- External evidence: `tcpreplay` (the standard PCAP replay tool) started as a simple packet re-emitter before growing protocol-aware features. Starting simple is validated practice.
- External evidence: Wireshark's "Follow TCP Stream" is essentially a text-mode replay and is one of its most-used features.

**Blast Radius:**
- Direct: replay engine module
- Ripple: CLI command structure (adds `--speed`, `--filter`, `--format` flags)
