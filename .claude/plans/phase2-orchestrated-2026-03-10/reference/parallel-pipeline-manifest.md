# Phase 2C: Multi-Core Parallel Pipeline — Deep Plan

**Goal**: Parallelize Probe's decode pipeline across CPU cores to achieve 10-100x
speedup on large captures. A 1GB pcap that takes Wireshark minutes should be
processed in seconds.

**Addresses**: Competitive Analysis recommendation #6 — Multi-Core Parallel Pipeline

**Why (data-backed)**:
- Wireshark is single-threaded and this is its #1 architectural limitation
  (GitLab #17468, Wiki/Wishlist)
- Wireshark takes 6 minutes to process a 36MB stream (GitLab #17313)
- CNCF: data volume is a top-3 challenge for 42% of teams
- Rust's ownership model + `rayon`/`tokio` make safe parallelism achievable
  where C (Wireshark) struggles
- Stanford's Retina framework processes 100+ Gbps in Rust with per-flow
  partitioning — the patterns are proven

**Scope**: Modifications to `prb-pcap`, `prb-core`, `prb-cli`. New benchmark
harness. ~2,500 lines of new/modified code.

**Prerequisites**: Phase 1 hardening complete (bug fixes in pipeline, TCP
reassembly correctness, TLS DSB bridge).

---

## Current Architecture (Single-Threaded)

```
PcapFileReader::read_all_packets()   ──── Eager load entire file into Vec<PcapPacket>
         │
         ▼
for packet in &packets {             ──── Sequential loop
  PacketNormalizer::normalize()      ──── STATEFUL (defrag pool)
         │
    ┌────┴────┐
    TCP       UDP → DebugEvent       ──── Stateless per-packet
    │
  TcpReassembler::process_segment() ──── STATEFUL (connections HashMap)
    │
  TlsStreamProcessor::process_stream() ── Stateless per-stream (keylog read-only)
    │
  create_debug_event_from_stream()   ──── Stateless
}
```

**Bottlenecks**:
1. Entire capture loaded into memory before processing begins
2. Single loop processes all packets sequentially
3. One TcpReassembler holds all connection state — no per-flow isolation
4. No protocol decoder dispatch in pipeline (gRPC/ZMQ/DDS decoders unused)
5. All events buffered in VecDeque before iteration

---

## Target Architecture (Multi-Core)

```
                    ┌──────────────────────┐
                    │  Memory-mapped file   │  mmap for zero-copy, constant
                    │  or streaming reader  │  memory regardless of file size
                    └──────────┬───────────┘
                               │ Vec<PcapPacket> (batch) or Stream<PcapPacket>
                               ▼
                    ┌──────────────────────┐
                    │  Parallel Normalize   │  rayon par_iter — stateless per
                    │  (non-frag packets)   │  non-fragmented packet; fragments
                    │                       │  routed to single-threaded defrag
                    └──────────┬───────────┘
                               │ Vec<NormalizedPacket>
                               ▼
                    ┌──────────────────────┐
                    │  Flow Partitioner     │  Hash 5-tuple → shard index
                    │  (scatter by flow)    │  Packets in same flow always go
                    │                       │  to same shard (ordering preserved)
                    └──────────┬───────────┘
                    ┌──────────┼──────────────────────────────────┐
                    ▼          ▼                                  ▼
              ┌──────────┐ ┌──────────┐                    ┌──────────┐
              │ Shard 0  │ │ Shard 1  │  ...               │ Shard N  │
              │ TCP reasm │ │ TCP reasm │                    │ TCP reasm │
              │ TLS dec   │ │ TLS dec   │                    │ TLS dec   │
              │ Proto dec  │ │ Proto dec  │                    │ Proto dec  │
              └─────┬─────┘ └─────┬─────┘                    └─────┬─────┘
                    │             │                                  │
                    └─────────────┼──────────────────────────────────┘
                                  ▼
                    ┌──────────────────────┐
                    │  Merge + Sort        │  Timestamp-ordered merge of
                    │  (gather)            │  per-shard event vectors
                    └──────────┬───────────┘
                               │ sorted Vec<DebugEvent>
                               ▼
                    ┌──────────────────────┐
                    │  Output (NDJSON/MCAP/ │
                    │  TUI EventStore)      │
                    └──────────────────────┘
```

---

## Library Choices

| Library | Version | Role | Why |
|---------|---------|------|-----|
| **rayon** | 1.11 | Batch data parallelism | Work-stealing, data-race freedom, minimal API change. 327M downloads. |
| **crossbeam-channel** | 0.5 | Pipeline stage channels | Mature MPMC, faster than std channels. For streaming mode. |
| **dashmap** | 6.1 | Concurrent flow table | Sharded locking, `&self` API, low contention. |
| **memmap2** | 0.9 | Memory-mapped file I/O | Zero-copy, constant memory, OS-managed paging. |
| **criterion** | 0.8 | Throughput benchmarks | Statistical microbenchmarks, regression detection. |
| **bytes** | 1.11 | Zero-copy buffers | Already in use; `Bytes::clone()` shares backing. |
| **ahash** | 0.8 | Fast flow hashing | Non-cryptographic, optimized for HashMap keys. |

---

## Subsection Index

| # | Subsection | Segments | Modified Crate | Est. Lines |
|---|-----------|----------|----------------|------------|
| 1 | Pipeline Trait Refactoring | 3 | `prb-core`, `prb-pcap` | ~400 |
| 2 | Parallel Packet Normalization | 2 | `prb-pcap` | ~300 |
| 3 | Flow-Partitioned TCP Reassembly | 3 | `prb-pcap` | ~450 |
| 4 | Parallel TLS Decryption + Protocol Decode | 2 | `prb-pcap` | ~250 |
| 5 | Memory-Mapped PCAP Reader | 2 | `prb-pcap` | ~300 |
| 6 | Streaming Pipeline Architecture | 3 | `prb-pcap`, `prb-core` | ~400 |
| 7 | Benchmarking Infrastructure | 2 | workspace root | ~250 |
| 8 | CLI Integration + Adaptive Parallelism | 2 | `prb-cli` | ~150 |

**Execution order**: S1 → S5 → S2 → S3 → S4 → S6 → S7 → S8

S1 must come first (trait bounds). S5 before S2 (reader feeds normalizer).
S2 → S3 → S4 are sequential (pipeline stages). S6 adds streaming on top of
batch. S7 and S8 are independent finishers.

---

## Parallelism Strategy Summary

### Stage 1: Normalization — Embarrassingly Parallel (with exception)

Non-fragmented packets are stateless: `(linktype, timestamp, &[u8]) → NormalizedPacket`.
These run through `rayon::par_iter`. IP fragments (rare, <1% of typical traffic)
are collected and processed single-threaded through the defrag pool.

### Stage 2: TCP Reassembly — Parallel per-Flow

Packets are partitioned by flow key (5-tuple hash). Each partition gets its own
`TcpReassembler`. Partitions are processed in parallel via `rayon::par_iter`.
No cross-flow synchronization needed — TCP state is strictly per-connection.

### Stage 3: TLS Decryption — Embarrassingly Parallel

Each `ReassembledStream` is independent. The `TlsKeyLog` is read-only after
initial load and can be shared via `Arc<TlsKeyLog>`. Each stream gets its own
`TlsDecryptor`. Perfect for `rayon::par_iter`.

### Stage 4: Protocol Decode — Parallel per-Stream

Each decrypted stream is independent. Protocol decoders are stateful per-connection
(H2Codec, ZmtpParser) but since streams are already per-connection, each gets
its own decoder instance. Perfect for `rayon::par_iter`.

### UDP Path — Embarrassingly Parallel

UDP datagrams are fully independent. Direct to `DebugEvent` via `rayon::par_iter`.

---

## Performance Targets

| Metric | Current (est.) | Target | Method |
|--------|---------------|--------|--------|
| 100MB pcap, 8 cores | ~5s | <0.5s | rayon batch |
| 1GB pcap, 8 cores | ~50s | <5s | mmap + rayon |
| 100k packets normalize | ~200ms | <30ms | rayon par_iter |
| Memory usage (1GB pcap) | ~2GB (full load) | ~50MB (mmap) | memmap2 |
| Throughput | ~200 Mpps | ~1.5 Gpps | all stages parallel |

---

## Risk Mitigation

| Risk | Mitigation |
|------|-----------|
| IP fragment reassembly is stateful | Route fragments to single-thread path; <1% of traffic |
| Packet ordering lost in parallel normalize | Each NormalizedPacket carries timestamp; sort after |
| Elephant flows saturate one shard | Monitor shard load; future: work-stealing within shard |
| `NormalizedPacket` borrows input data | Switch to owned `OwnedNormalizedPacket` for cross-thread |
| rayon overhead for small captures | Adaptive: use sequential path for <10k packets |
| Benchmark reproducibility | Use synthetic pcap fixtures with deterministic content |

---

## Acceptance Criteria

- [ ] `cargo build --workspace` — zero errors, zero warnings
- [ ] `cargo clippy --workspace --all-targets` — zero warnings
- [ ] `cargo test --workspace` — all tests pass (existing + new)
- [ ] `prb ingest large.pcap --jobs 0` uses all available cores
- [ ] `prb ingest large.pcap --jobs 1` falls back to sequential
- [ ] 1GB pcap processed in <5s on 8-core machine (benchmark)
- [ ] Memory usage for 1GB pcap stays under 100MB (mmap path)
- [ ] Benchmark suite in `benches/` with criterion
- [ ] Output is deterministic: same input → same events regardless of job count
- [ ] No unsafe code outside of memmap2 (which is inherently unsafe)
- [ ] Pipeline stats (packets_read, tcp_streams, etc.) are correct under parallelism
- [ ] All existing tests pass unchanged (backward compatible)
