---
plan: "Universal Message Debugger -- Phase 1 Decomposition Plan"
goal: "Decompose the Phase 1 CLI message debugger into 5 independently plannable subsections, incorporating gaps found during research verification"
generated: 2026-03-08
status: Ready for execution
parent_plan: ""
rules_version: 2026-03-08
---

# Universal Message Debugger -- Phase 1 -- Manifest

## Dependency Diagram

```
Subsection 1: Foundation & Core Model
        │
        ▼
Subsection 2: Storage & Schema Engine
        │
        ▼
Subsection 3: Network Capture Pipeline
        │
        ▼
Subsection 4: Protocol Decoders
        │
        ▼
Subsection 5: Analysis & Replay
```

Subsections are strictly sequential. Each one defines types, traits, or capabilities consumed by the next. No parallelization at the subsection level is possible because each depends on the full output of its predecessor.

Within each subsection, the subsequent deep-plan will identify parallelizable segments.

## Subsection Index

| # | Title | Directory | Issues Addressed | Risk | Complexity | Status |
|---|-------|-----------|-----------------|------|------------|--------|
| 1 | Foundation & Core Model | subsection-1-foundation-core-model/ | 4, 11 | 3/10 | Medium | pending |
| 2 | Storage & Schema Engine | subsection-2-storage-schema-engine/ | 6 | 4/10 | Medium | pending |
| 3 | Network Capture Pipeline | subsection-3-network-capture-pipeline/ | 1, 5, 7, 10 | 7/10 | High | pending |
| 4 | Protocol Decoders | subsection-4-protocol-decoders/ | 2, 3, 9 | 7/10 | High | pending |
| 5 | Analysis & Replay | subsection-5-analysis-replay/ | 8, 9, 12 | 5/10 | Medium | pending |

## Parallelization

No parallelization at the subsection level. All 5 subsections are strictly sequential due to type/trait dependencies.

Within each subsection, the respective deep-plan identifies parallelizable segments:
- Subsection 1: no parallelism (3 sequential segments)
- Subsection 2: Segments 3 and 4 are parallelizable after Segment 2
- Subsection 3: no parallelism (5 sequential segments)
- Subsection 4: Segments 2 and 3 parallelizable after Segment 1
- Subsection 5: Segments 2, 3, 4, 5 parallelizable after Segment 1; Segment 6 floats freely

## Revised Protocol Scope for Phase 1

### In scope (offline, PCAP-based):
- gRPC over HTTP/2 (TCP, with TLS decryption when key material available)
- ZMQ/ZMTP (TCP)
- DDS/RTPS (UDP)
- Raw TCP streams (generic, no protocol decoding)
- Raw UDP datagrams (generic)

### Moved to Phase 2 (requires live capture agent):
- Iceoryx shared memory
- Unix domain sockets
- POSIX shared memory
- Named pipes
- Custom protobuf pub/sub over IPC

### Moved to later phases:
- Kafka, MQTT, QUIC/HTTP3, eBPF capture

## Revised Library Dependencies

| Original Plan | Revised | Reason |
|---|---|---|
| `h2` | `h2-sans-io` | `h2` is async client/server; `h2-sans-io` is offline frame parser |
| `pcap-file` + `pcap-parser` | `pcap-parser` only | `pcap-parser` handles both formats; `pcap-file` is older and redundant |
| (not mentioned) | `tls-parser` | TLS record parsing for decryption pipeline |
| (not mentioned) | `ring` v0.17+ | Symmetric crypto for TLS record decryption and key derivation |
| (not mentioned) | `smoltcp` v0.12+ | TCP reassembly via `storage::Assembler` |
| (not mentioned) | `criterion` | Benchmarking framework |
| (not mentioned) | `rtps-parser` v0.1.1 | RTPS/DDS packet parsing |
| `zmtp` | Custom ZMTP parser | `zmtp` crate is dead (last updated 2016) |
| (not mentioned) | `flate2` | gRPC message decompression |

## Preamble Injection

Before launching any builder subagent, the orchestration agent assembles the prompt:
1. Read `iterative-builder-prompt.mdc` from `.cursor/rules/`
2. Read `devcontainer-exec.mdc` from `.cursor/rules/` (if applicable)
3. Read the segment file from the appropriate subsection's `segments/{NN}-{slug}.md`

Assembled prompt = [preamble contents] + [segment file contents]

## Execution Instructions

This is a two-level plan. For each subsection in order (1 through 5):
1. Navigate to the subsection's restructured directory, read its manifest.
2. Execute segments per the subsection manifest's instructions.
3. After all segments in a subsection pass, run deep-verify against the subsection plan.
4. If verification finds gaps, re-enter deep-plan on the unresolved items before proceeding.

Do not begin a later subsection until the previous subsection's traits, types, and interfaces are implemented and stable.

## Risk Budget Assessment

Two subsections (3 and 4) are at risk 7/10. Mitigations:
- Subsection 3: TLS decryption can be deferred to Phase 1.5 without breaking the pipeline.
- Subsection 4: Each protocol decoder is a separate segment; if DDS/RTPS proves intractable, gRPC and ZMQ still ship.
