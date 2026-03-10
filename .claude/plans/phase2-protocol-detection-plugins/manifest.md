---
plan: "Phase 2 — Protocol Auto-Detection & Extensible Decoder Plugin System"
goal: "Wire existing decoders into the PCAP pipeline with automatic protocol detection, then build native and WASM plugin systems so users can add custom protocol decoders"
generated: 2026-03-10
status: Ready for execution
parent_plan: ".claude/plans/prb-phase1-master-2026-03-09.md"
competitive_analysis_ref: ".claude/research/competitive-analysis-2026-03-10.md (Recommendation #7)"
rules_version: 2026-03-10
---

# Phase 2 — Protocol Auto-Detection & Extensible Decoder Plugin System

## Problem Statement

The pipeline today emits `RawTcp` / `RawUdp` events with undecoded payloads. Three
working protocol decoders (`GrpcDecoder`, `ZmqDecoder`, `DdsDecoder`) exist in
separate crates but are **not wired into the ingest pipeline**. There is no:

1. Mechanism to detect which protocol a TCP/UDP stream carries
2. Registry that maps detected protocols to decoder instances
3. Dispatch layer that routes streams to the correct decoder
4. Plugin API for users to add custom decoders
5. Plugin management (install, discover, load)

This plan addresses all five gaps in six sequential segments.

## Key Design Decisions

### D1: Layered Detection (Wireshark-inspired)

Wireshark's dissector architecture uses three dispatch levels in order:

1. **Port-based table lookup** — fastest, but requires known ports
2. **Magic-byte / header inspection** — inspects first N bytes of payload
3. **Heuristic dissectors** — deeper content analysis, offered sequentially

We adopt the same layered approach:

```
┌─────────────────────────────────────┐
│  1. User override (--protocol)      │  Highest priority
├─────────────────────────────────────┤
│  2. Port mapping table              │  O(1) lookup
├─────────────────────────────────────┤
│  3. Magic-byte detection (guess)    │  Inspects first 64 bytes
├─────────────────────────────────────┤
│  4. Heuristic detectors (per-proto) │  Deeper analysis, tried in order
├─────────────────────────────────────┤
│  5. Fallback → RawTcp / RawUdp      │  Lowest priority
└─────────────────────────────────────┘
```

### D2: Zero-Copy Detection with `guess` Crate

The `guess` crate (v0.2, 2026-02-07) provides zero-copy, no-alloc protocol
detection from initial bytes. Supports HTTP/2, TLS, SSH, DNS, and 20+ protocols.
We use it for layer 3 (magic-byte detection) and supplement with custom detectors
for ZMTP and RTPS which `guess` does not cover.

### D3: Registry as Central Coordination Point

A `DecoderRegistry` owns all detector + decoder pairs. The pipeline calls
`registry.detect_and_decode(stream)` — one method, one responsibility boundary.
The registry is protocol-agnostic; built-in and plugin decoders are registered
identically.

### D4: Dual Plugin Backend — Native + WASM

| Backend | Use Case | Safety | Performance | Portability |
|---------|----------|--------|-------------|-------------|
| **Native** (`libloading`) | Trusted first-party / performance-critical decoders | Unsafe (same process) | Full native speed | Platform-specific (.so/.dylib/.dll) |
| **WASM** (`extism`) | Community/third-party decoders, sandboxed execution | Sandboxed | ~2-5x overhead | Portable (.wasm) |

Both backends implement the same `PluginDecoder` trait adapter, so the registry
treats them identically.

### D5: Plugin Contract via `prb-plugin-api` Crate

A thin, stable crate (`prb-plugin-api`) defines the ABI contract:
- Serialized `DetectRequest` → `DetectResponse` (can this plugin handle the stream?)
- Serialized `DecodeRequest` → `DecodeResponse` (decode the stream)
- JSON serialization for WASM boundary, zero-copy `&[u8]` for native

This crate is versioned independently and follows semver strictly. Plugins depend
only on this crate, not on `prb-core`.

## Dependency Diagram

```
Segment 1: ProtocolDetector Trait + Built-in Detectors
        │
        ▼
Segment 2: DecoderRegistry + Dispatch Layer
        │
        ▼
Segment 3: Pipeline Integration (Wire Decoders into PcapCaptureAdapter)
        │
        ▼
Segment 4: Native Plugin System (libloading)
        │
        ▼
Segment 5: WASM Plugin System (extism)
        │
        ▼
Segment 6: Plugin Management CLI
```

Segments 1-3 are the critical path — they deliver the core value (auto-detection
and decoded output) without any plugin infrastructure. Segments 4-6 are the
extensibility layer.

**Parallelization:** Segments 4 and 5 can run in parallel after Segment 3
completes. Segment 6 depends on both 4 and 5.

## Segment Index

| # | Title | File | New Crates | Dependencies | Risk | Complexity |
|---|-------|------|------------|-------------|------|------------|
| 1 | ProtocolDetector Trait + Built-in Detectors | `segments/01-protocol-detector.md` | `prb-detect` | `guess`, `prb-core` | 3/10 | Medium |
| 2 | DecoderRegistry + Dispatch Layer | `segments/02-decoder-registry.md` | (extends `prb-detect`) | `prb-core`, `prb-detect` | 4/10 | Medium |
| 3 | Pipeline Integration | `segments/03-pipeline-integration.md` | (modifies `prb-pcap`, `prb-cli`) | `prb-detect`, `prb-grpc`, `prb-zmq`, `prb-dds` | 6/10 | High |
| 4 | Native Plugin System | `segments/04-native-plugins.md` | `prb-plugin-api`, `prb-plugin-native` | `libloading` | 5/10 | Medium |
| 5 | WASM Plugin System | `segments/05-wasm-plugins.md` | `prb-plugin-wasm` | `extism` | 6/10 | High |
| 6 | Plugin Management CLI | `segments/06-plugin-cli.md` | (extends `prb-cli`) | `prb-plugin-native`, `prb-plugin-wasm` | 3/10 | Low |

## Issue Index

| # | Title | File | Severity | Segments Affected |
|---|-------|------|----------|-------------------|
| 1 | ZMTP/RTPS not covered by `guess` crate | `issues/issue-01-custom-detectors.md` | Medium | 1 |
| 2 | Mid-stream capture detection failures | `issues/issue-02-mid-stream-detection.md` | High | 1, 2, 3 |
| 3 | ABI stability for native plugins | `issues/issue-03-native-abi-stability.md` | High | 4 |
| 4 | WASM memory and CPU limits | `issues/issue-04-wasm-resource-limits.md` | Medium | 5 |
| 5 | Decoder state management across stream chunks | `issues/issue-05-stateful-decoders.md` | High | 2, 3 |

## New Workspace Crates

After Phase 2, the workspace gains these crates:

```
crates/
├── prb-detect/          # Protocol detection + decoder registry
│   ├── src/
│   │   ├── lib.rs
│   │   ├── detector.rs  # ProtocolDetector trait + built-ins
│   │   ├── registry.rs  # DecoderRegistry
│   │   └── dispatch.rs  # Dispatch logic (layered detection)
│   └── Cargo.toml
├── prb-plugin-api/      # Stable ABI contract for plugins
│   ├── src/
│   │   ├── lib.rs
│   │   ├── types.rs     # DetectRequest/Response, DecodeRequest/Response
│   │   └── version.rs   # API version negotiation
│   └── Cargo.toml
├── prb-plugin-native/   # Native (.so/.dylib) plugin loader
│   ├── src/
│   │   ├── lib.rs
│   │   ├── loader.rs    # libloading wrapper
│   │   └── adapter.rs   # PluginDecoder → ProtocolDecoder adapter
│   └── Cargo.toml
└── prb-plugin-wasm/     # WASM plugin loader
    ├── src/
    │   ├── lib.rs
    │   ├── runtime.rs   # extism host setup
    │   └── adapter.rs   # WASM → ProtocolDecoder adapter
    └── Cargo.toml
```

## New Dependencies

| Crate | Version | Purpose | Used By |
|-------|---------|---------|---------|
| `guess` | 0.2 | Zero-copy protocol detection | `prb-detect` |
| `libloading` | 0.8+ | Dynamic library loading | `prb-plugin-native` |
| `extism` | 1.10+ | WASM plugin runtime | `prb-plugin-wasm` |
| `extism-pdk` | (plugin-side) | WASM plugin development kit | Plugin authors |
| `semver` | 1 | API version comparison | `prb-plugin-api` |
| `toml` | 0.8+ | Plugin manifest parsing | `prb-plugin-native`, `prb-plugin-wasm` |

## Execution Instructions

Execute segments 1 through 6 in order (with 4 and 5 parallelizable).

For each segment:
1. Read the segment file from `segments/NN-slug.md`
2. Execute all tasks in the segment
3. Run `cargo test --workspace` — all tests must pass
4. Run `cargo clippy --workspace -- -D warnings` — no warnings
5. Mark the segment complete in the execution log

After all segments pass, run end-to-end validation:
```
prb ingest fixtures/grpc_sample.json   # JSON fixture path (existing)
prb ingest capture.pcap                # PCAP with mixed protocols (new fixture)
prb plugins list                       # Shows built-in + loaded plugins
```

## Success Criteria

1. **Auto-detection works**: `prb ingest capture.pcap` produces `Grpc`, `Zmq`, and
   `DdsRtps` events (not `RawTcp`/`RawUdp`) without any `--protocol` flag
2. **Fallback works**: Unknown protocols still produce `RawTcp`/`RawUdp` events
   with warnings
3. **User override works**: `--protocol grpc` forces gRPC decoding regardless of
   detection result
4. **Native plugins load**: A sample native decoder plugin can be compiled and loaded
5. **WASM plugins load**: A sample WASM decoder plugin can be compiled and loaded
6. **CLI management**: `prb plugins list`, `prb plugins info <name>` work
7. **No regression**: All existing Phase 1 tests continue to pass
8. **Performance**: Detection adds <1μs per stream (benchmark required)
