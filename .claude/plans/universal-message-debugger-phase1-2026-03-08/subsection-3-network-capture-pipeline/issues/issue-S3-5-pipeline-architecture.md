---
id: "S3-5"
title: "Pipeline Architecture and Data Flow"
risk: 4/10
addressed_by_segments: [5]
---

# Issue S3-5: Pipeline Architecture and Data Flow

**Core Problem:**
The four components (file reader, packet normalizer, TCP reassembler, TLS decryptor) must be composed into a coherent pipeline that produces reassembled byte streams suitable for Subsection 4's protocol decoders. The pipeline must handle errors gracefully without aborting the entire ingest.

**Root Cause:**
Each component has its own data model and error modes. Integration requires defining clear interfaces between stages.

**Proposed Fix:**
Define a layered pipeline with explicit stage boundaries:

```
File Reader -> [raw packets + metadata]
  -> Packet Normalizer -> [IP datagrams, defragmented]
    -> TCP Reassembler -> [byte streams per connection]
      -> TLS Decryptor (optional) -> [plaintext byte streams]
    -> UDP Extractor -> [datagrams per src:dst pair]
```

Each stage produces typed output consumed by the next. Errors at any stage produce warnings (logged via `tracing`) and skip the affected packet/stream, not abort the pipeline. The pipeline implements the `CaptureAdapter` trait from Subsection 1.

**Existing Solutions Evaluated:**
N/A -- internal architecture design. No external tool solves "compose our custom pipeline stages."

**Alternatives Considered:**
- Pull-based (iterator) pipeline -- rejected for TCP reassembly which needs to buffer across packets.
- Separate binary for PCAP ingest -- rejected, should be unified CLI.

**Pre-Mortem -- What Could Go Wrong:**
- Stage boundaries may cause unnecessary copies if not designed with zero-copy in mind (use `bytes::Bytes` for shared ownership).
- Streaming vs. batch tradeoff: streaming is more memory-efficient but harder for TCP reassembly (need all packets for a connection).
- Large captures (multi-GB) need bounded memory usage; must not load entire file.

**Risk Factor:** 4/10

**Evidence for Optimality:**
- Existing solutions: `pcapsql-core` follows the same layered architecture (file -> packet -> stream -> TLS -> protocol), validating the design.
- External evidence: Wireshark's dissector pipeline follows identical layering (link -> network -> transport -> application).

**Blast Radius:**
- Direct: pipeline orchestration module in `prb-pcap`
- Ripple: CLI integration (`prb ingest`), all protocol decoders consume pipeline output
