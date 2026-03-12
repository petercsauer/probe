# Phase 2 — Live Capture Mode: Deep Plan

**Goal**: Add `prb capture -i eth0` with real-time protocol decoding, eliminating
the tcpdump→file→analyze two-step. Users get live, decoded gRPC/ZMQ/DDS-RTPS
visibility in their terminal — the value proposition that earned Hubble 14K+ stars,
but for any protocol prb already decodes, on any interface, without Kubernetes.

**Scope**: One new crate (`prb-capture`), modifications to three existing crates
(`prb-pcap`, `prb-core`, `prb-cli`), deep integration with the Phase 2 TUI.
~3,000 lines of new code.

---

## Architecture Overview

```
                  ┌──────────────────────────────────────────────────────┐
                  │                  prb-capture crate                   │
                  │                                                      │
 prb capture ────►│  privileges::check()                                │
 -i eth0         │       │                                              │
 -f "tcp 443"    │       ▼                                              │
                  │  InterfaceEnumerator ──► pcap::Device::list()       │
                  │       │                                              │
                  │       ▼                                              │
                  │  CaptureEngine                                      │
                  │  ├── pcap::Capture::from_device(iface)              │
                  │  │   .promisc(true)                                 │
                  │  │   .snaplen(65535)                                │
                  │  │   .immediate_mode(true)                          │
                  │  │   .buffer_size(16MB)                             │
                  │  │   .open()?                                       │
                  │  ├── cap.filter(bpf_expr, true)?                    │
                  │  └── OS thread: loop { cap.next_packet() }          │
                  │           │                                          │
                  │           │ crossbeam::bounded(8192)                 │
                  │           ▼                                          │
                  │  PacketDispatcher                                   │
                  │  ├── etherparse normalize (reuse PacketNormalizer)  │
                  │  ├── TCP reassembly (reuse TcpReassembler)          │
                  │  ├── TLS decrypt (reuse TlsStreamProcessor)        │
                  │  └── Protocol decode (gRPC/ZMQ/DDS)                │
                  │           │                                          │
                  │           │ tokio::mpsc::channel(4096)              │
                  │           ▼                                          │
                  │  LiveCaptureAdapter (implements CaptureAdapter)     │
                  └──────────┬───────────────────────────────────────────┘
                             │
                ┌────────────┼────────────────┐
                ▼            ▼                ▼
         ┌──────────┐ ┌──────────────┐ ┌──────────────┐
         │ TUI live │ │ NDJSON       │ │ pcap save    │
         │ ratatui  │ │ streaming    │ │ -w file.pcap │
         │ (phase2) │ │ to stdout    │ │              │
         └──────────┘ └──────────────┘ └──────────────┘
```

**Key insight**: The existing `prb-pcap` pipeline (normalize → reassemble → TLS →
event) was designed for batch processing. For live capture, we reuse every internal
component but replace the packet *source* (file → live interface) and the event
*sink* (VecDeque → streaming channel). The pipeline itself is unchanged.

---

## Design Principles

1. **Reuse over rewrite**: Every existing pipeline stage (PacketNormalizer,
   TcpReassembler, TlsStreamProcessor) is reused. Only the capture source changes.
2. **Dedicated capture thread**: Following Hubble/Suricata/Zeek's universal pattern,
   the capture runs on a real OS thread to guarantee no packet loss from async
   scheduling. Decode happens on tokio tasks.
3. **Accept loss, report it**: All production capture tools (Hubble, Suricata, Zeek,
   tcpdump) accept packet loss under overload and report statistics. We do the same.
4. **Trusted libraries only**: `pcap` crate (4.18M downloads, wraps libpcap),
   `crossbeam-channel` (600M+ downloads), `etherparse` (already in workspace).
5. **Privilege-aware**: Detect capabilities at startup, provide clear `setcap`
   instructions, drop privileges after opening capture device.

---

## State of the Art — Lessons Applied

| System | Lesson | How prb applies it |
|--------|--------|--------------------|
| **Hubble** (Cilium, 14K★) | Ring buffer with evict-oldest; gRPC streaming API | Display ring buffer for TUI; future gRPC output |
| **Suricata** (9K★) | Workers mode: each thread = full pipeline; AF_PACKET ring | Single capture thread + channel-fed decode workers |
| **Zeek 8.x** (7K★) | Separate capture from analysis; ZeroMQ pub/sub | OS thread capture → crossbeam channel → tokio decode |
| **Retina** (Stanford) | Subscription-based: only decode what's needed | BPF capture filter + application display filter |
| **Wireshark/tshark** | Capture filter vs display filter separation | BPF at kernel (cheap) + prb-query at app (rich) |
| **Termshark** (3K★) | TUI live capture UX: list + decode + hex + sparkline | Integrate with existing phase2 TUI panes |
| **tcpdump/libpcap** | PACKET_MMAP (TPACKET_V3) for zero-copy ring buffers | Automatic via libpcap ≥1.5 through `pcap` crate |

---

## Dependency Matrix

### New Dependencies

| Purpose | Crate | Version | Downloads | Justification |
|---------|-------|---------|-----------|---------------|
| Live capture | `pcap` | 2.4 | 4.18M | De facto Rust libpcap wrapper; BPF, async, cross-platform |
| Pipeline channels | `crossbeam-channel` | 0.5 | 600M+ | Lock-free bounded MPMC; backpressure via `try_send` |
| Async runtime | `tokio` | 1.x | 800M+ | Already used by ratatui event loop in phase2 TUI |
| Privilege mgmt | `caps` | 0.5 | 200K+ | Linux capability get/set/drop; lightweight |

### Existing Dependencies (reused)

| Crate | Used For |
|-------|----------|
| `etherparse` 0.19 | Packet parsing (already in prb-pcap) |
| `smoltcp` 0.12 | TCP reassembly (already in prb-pcap) |
| `tls-parser` 0.12 | TLS handshake parsing (already in prb-pcap) |
| `ring` 0.17 | TLS decryption (already in prb-pcap) |
| `ratatui` 0.30 | TUI (from phase2 TUI subsections) |
| `crossterm` | Terminal backend (from phase2 TUI) |
| `clap` 4 | CLI (existing) |

---

## Subsection Index

| # | Subsection | Segments | Crate(s) | Est. Lines |
|---|-----------|----------|----------|------------|
| 1 | Capture Engine | 4 | `prb-capture` (new) | ~800 |
| 2 | Live Pipeline Integration | 3 | `prb-pcap` refactor | ~400 |
| 3 | CLI Integration | 3 | `prb-cli` | ~350 |
| 4 | TUI Live Mode | 3 | `prb-tui` | ~600 |
| 5 | Output Sinks | 3 | `prb-capture` | ~350 |
| 6 | Privilege Management | 2 | `prb-capture` | ~200 |
| 7 | Testing Strategy | 3 | all | ~300 |

**Execution order**: S6 → S1 → S2 → S3 → S5 → S4 → S7

S6 (privileges) must come first — can't open a capture device without it.
S1 (engine) provides the core capture loop.
S2 (pipeline) refactors prb-pcap internals for streaming reuse.
S3 (CLI) wires `prb capture` command.
S5 (sinks) adds output modes.
S4 (TUI) integrates with the existing phase2 TUI panes.
S7 (testing) validates everything end-to-end.

---

## Subsection Details

### S1: Capture Engine (`prb-capture`)

See: `subsection-1-capture-engine.md`

New crate wrapping the `pcap` crate for live packet capture. Provides interface
enumeration, BPF filter compilation, a dedicated capture thread with ring buffer
delivery, and capture statistics (drops, packet counts).

**Segments**:
- S1.1: Crate scaffold + `pcap` integration + `CaptureConfig`
- S1.2: `CaptureEngine` — OS thread capture loop + crossbeam channel
- S1.3: `InterfaceEnumerator` — device listing with flags/addresses
- S1.4: `CaptureStats` — kernel drops, channel drops, packet/byte counters

### S2: Live Pipeline Integration

See: `subsection-2-live-pipeline.md`

Refactor `prb-pcap`'s internal pipeline stages (`PacketNormalizer`,
`TcpReassembler`, `TlsStreamProcessor`) from batch-only to streaming-capable.
Create `LiveCaptureAdapter` implementing `CaptureAdapter` for live sources.

**Segments**:
- S2.1: Extract `PipelineCore` — shared normalize→reassemble→TLS→event logic
- S2.2: `LiveCaptureAdapter` — streaming `CaptureAdapter` implementation
- S2.3: Protocol decoder integration — wire gRPC/ZMQ/DDS decoders into live path

### S3: CLI Integration

See: `subsection-3-cli-integration.md`

Add `prb capture` subcommand with interface selection, BPF filters, output mode,
snap length, promiscuous mode, and capture duration controls.

**Segments**:
- S3.1: `CaptureArgs` struct + `Commands::Capture` variant
- S3.2: `run_capture()` — orchestration: config → engine → pipeline → sink
- S3.3: Interface list subcommand: `prb capture --list-interfaces`

### S4: TUI Live Mode

See: `subsection-4-tui-live-mode.md`

Extend the phase2 TUI to support live capture as a data source. The TUI receives
`DebugEvent`s via a tokio channel and appends them to the event list in real time.
Adds a capture control bar (start/stop/pause), live stats display, and auto-scroll.

**Segments**:
- S4.1: `LiveDataSource` — tokio channel → EventStore bridge
- S4.2: Capture control bar (start/stop/pause) + stats overlay
- S4.3: Auto-scroll + rate limiting for high-throughput display

### S5: Output Sinks

See: `subsection-5-output-sinks.md`

Multiple output modes for captured data: streaming NDJSON to stdout/file, pcap
savefile (write raw packets for later analysis), and MCAP session recording.

**Segments**:
- S5.1: `OutputSink` trait + `NdjsonSink` (streaming to stdout/file)
- S5.2: `PcapSaveSink` — write captured packets to pcap/pcapng file
- S5.3: `McapSink` — write decoded events to MCAP session file

### S6: Privilege Management

See: `subsection-6-privilege-mgmt.md`

Cross-platform privilege detection, clear error messages with remediation steps,
and post-capture privilege dropping for defense-in-depth.

**Segments**:
- S6.1: Capability detection + user-friendly error messages
- S6.2: Privilege dropping after capture device open + container support

### S7: Testing Strategy

See: `subsection-7-testing.md`

Test strategy spanning unit tests (mock capture), integration tests (loopback
interface capture), and platform-specific CI configuration.

**Segments**:
- S7.1: Unit tests with mock packet sources
- S7.2: Integration tests on loopback interface
- S7.3: CI configuration + platform matrix

---

## Key Technical Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Capture library | `pcap` 2.4 | Wraps libpcap; PACKET_MMAP automatic; BPF built-in; 4.18M downloads |
| Capture thread model | Dedicated OS thread | Universal pattern (Hubble, Suricata, Zeek): real thread guarantees no scheduler-induced drops |
| Capture→decode channel | `crossbeam-channel` bounded(8192) | Lock-free MPMC; backpressure via `try_send`; drop + count on overflow |
| Decode→output channel | `tokio::mpsc` channel(4096) | Integrates with async TUI event loop; bounded for backpressure |
| Pipeline reuse | Extract shared `PipelineCore` from `PcapCaptureAdapter` | Avoids duplicating normalize/reassemble/TLS/decode logic |
| Backpressure model | Drop oldest + count (Hubble model) | Industry standard; all tools accept loss under overload |
| Privilege approach | `caps` crate + `setcap` instructions | Avoids running as root; follows least-privilege principle |
| BPF vs app filter | BPF at kernel + prb-query at display | BPF reduces kernel→user copy; prb-query provides rich field-level filtering |
| Snaplen default | 65535 | Must capture full gRPC frames (can be large); user can reduce via `--snaplen` |
| Buffer size default | 16 MB | ~10ms at 1Gbps; handles typical burst without drops |

---

## Data Flow: Live Capture Path

```
Network Interface (eth0, lo, etc.)
    │
    │  BPF filter (kernel space, zero-copy via PACKET_MMAP)
    ▼
pcap::Capture::next_packet()           [OS thread, blocking]
    │
    │  crossbeam::bounded(8192) — try_send, drop if full
    ▼
PacketNormalizer::normalize()           [tokio task]
    │
    ├── UDP → DebugEvent (immediate)
    │
    └── TCP → TcpReassembler::process_segment()
              │
              ├── ReassembledStream → TlsStreamProcessor
              │       │
              │       └── DecryptedStream → ProtocolDecoder
              │               │
              │               └── DebugEvent
              │
              └── StreamEvent::GapSkipped → warning
    │
    │  tokio::mpsc::channel(4096)
    ▼
Output Sink (one of):
    ├── TUI: append to EventStore, render
    ├── NDJSON: serialize + write line
    ├── Pcap savefile: write raw packet
    └── MCAP: write decoded event
```

---

## Acceptance Criteria

- [ ] `prb capture -i lo` captures loopback traffic and displays decoded events
- [ ] `prb capture -i eth0 -f "tcp port 50051"` filters gRPC traffic via BPF
- [ ] `prb capture --list-interfaces` shows all available interfaces with addresses
- [ ] `prb capture -i eth0 -w capture.pcap` saves raw packets to file
- [ ] `prb capture -i eth0 --tui` opens TUI with live scrolling event list
- [ ] Live capture decodes gRPC, ZMQ, and DDS-RTPS protocols in real time
- [ ] Without root/capabilities: clear error message with `setcap` instructions
- [ ] Ctrl+C cleanly stops capture, prints summary statistics
- [ ] Drop statistics reported: kernel drops (`pcap_stats`), channel drops
- [ ] Handles sustained 10k pps without drops on modern hardware
- [ ] `cargo build --workspace` — zero errors with new crate
- [ ] `cargo clippy --workspace --all-targets` — zero warnings
- [ ] `cargo test --workspace` — all tests pass (including loopback capture tests)
- [ ] Works on Linux (primary) and macOS (secondary); Windows documented as unsupported
