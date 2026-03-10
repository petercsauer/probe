---
segment: 18
title: "Parallel Packet Normalization"
depends_on: [16, 17]
risk: 9
complexity: Medium
cycle_budget: 3
status: pending
commit_message: "feat(prb-pcap): add rayon-parallelized packet normalization with fragment fallback"
---

# Subsection 2: Parallel Packet Normalization

## Purpose

Parallelize the first pipeline stage — converting raw `PcapPacket`s into
`OwnedNormalizedPacket`s — across all CPU cores using rayon. This stage handles
linktype dispatch (Ethernet, SLL, SLL2, Raw IP, Loopback), VLAN stripping, and
transport header extraction.

---

## Analysis: What Can Be Parallelized?

The normalizer has two code paths:

### Non-fragmented packets (>99% of typical traffic)

These are **stateless**: each packet's normalization depends only on its own
bytes. The function signature is effectively:

```
(linktype: u32, timestamp: u64, data: &[u8]) → Result<OwnedNormalizedPacket>
```

This is embarrassingly parallel — perfect for `rayon::par_iter().map()`.

### IP fragments (<1% of typical traffic)

These are **stateful**: fragments from the same IP datagram must be collected
in the `IpDefragPool` before the transport layer can be parsed. Fragment
reassembly requires ordering by fragment offset and waiting for all fragments.

Strategy: **Two-pass approach**. First pass (parallel) normalizes all
non-fragmented packets. Second pass (sequential) handles fragments through the
existing defrag pool.

---

## Segment S2.1: Stateless Normalization Function

Extract the non-fragmented normalization logic into a pure, thread-safe function:

```rust
// crates/prb-pcap/src/normalize.rs

/// Normalizes a single non-fragmented packet. Returns None if the packet
/// is an IP fragment that requires stateful reassembly.
///
/// This function is stateless and safe to call from multiple threads.
pub fn normalize_stateless(
    linktype: u32,
    timestamp_us: u64,
    data: &[u8],
) -> Result<NormalizeResult, PcapError> {
    let sliced = match linktype {
        0 => parse_loopback_static(data)?,
        1 => SlicedPacket::from_ethernet(data)
            .map_err(|e| PcapError::Parse(format!("Ethernet parse: {:?}", e)))?,
        101 => SlicedPacket::from_ip(data)
            .map_err(|e| PcapError::Parse(format!("Raw IP parse: {:?}", e)))?,
        113 => SlicedPacket::from_linux_sll(data)
            .map_err(|e| PcapError::Parse(format!("SLL parse: {:?}", e)))?,
        276 => parse_sll2_static(data)?,
        _ => return Err(PcapError::InvalidLinktype(format!("unsupported: {}", linktype))),
    };

    let vlan_id = sliced.vlan_ids().first().map(|v| v.value());

    let net = sliced.net.as_ref()
        .ok_or_else(|| PcapError::Parse("no network layer".into()))?;

    let is_fragmented = match net {
        NetSlice::Ipv4(ipv4) => ipv4.payload().fragmented,
        NetSlice::Ipv6(ipv6) => ipv6.payload().fragmented,
        NetSlice::Arp(_) => return Err(PcapError::Parse("ARP not supported".into())),
    };

    if is_fragmented {
        return Ok(NormalizeResult::Fragment {
            timestamp_us,
            linktype,
            data_len: data.len(),
        });
    }

    // Non-fragmented: extract everything
    let (src_ip, dst_ip) = extract_ips(net);
    let (transport, payload) = extract_transport(&sliced)?;

    Ok(NormalizeResult::Packet(OwnedNormalizedPacket {
        timestamp_us,
        src_ip,
        dst_ip,
        transport,
        vlan_id,
        payload: payload.to_vec(),
    }))
}

pub enum NormalizeResult {
    Packet(OwnedNormalizedPacket),
    Fragment {
        timestamp_us: u64,
        linktype: u32,
        data_len: usize,
    },
}
```

The helper functions `parse_loopback_static`, `parse_sll2_static`,
`extract_ips`, and `extract_transport` are extracted from the existing
`PacketNormalizer` methods, removing `&self` dependency.

---

## Segment S2.2: Parallel Batch Normalization

Wire into the `ParallelPipeline` orchestrator:

```rust
// crates/prb-pcap/src/parallel/normalize.rs

use rayon::prelude::*;

pub struct NormalizeBatch;

impl NormalizeBatch {
    /// Normalizes a batch of packets in parallel. Returns normalized packets
    /// and indices of fragments that need sequential processing.
    pub fn run(
        packets: &[PcapPacket],
    ) -> (Vec<OwnedNormalizedPacket>, Vec<usize>) {
        let results: Vec<(usize, Result<NormalizeResult, PcapError>)> = packets
            .par_iter()
            .enumerate()
            .map(|(idx, pkt)| {
                (idx, normalize_stateless(pkt.linktype, pkt.timestamp_us, &pkt.data))
            })
            .collect();

        let mut normalized = Vec::with_capacity(packets.len());
        let mut fragment_indices = Vec::new();
        let mut failed = 0u64;

        for (idx, result) in results {
            match result {
                Ok(NormalizeResult::Packet(pkt)) => normalized.push(pkt),
                Ok(NormalizeResult::Fragment { .. }) => fragment_indices.push(idx),
                Err(e) => {
                    failed += 1;
                    tracing::warn!("Normalize failed for packet {}: {}", idx, e);
                }
            }
        }

        tracing::debug!(
            "Parallel normalize: {} packets, {} fragments, {} failed",
            normalized.len(),
            fragment_indices.len(),
            failed
        );

        (normalized, fragment_indices)
    }
}
```

### Fragment handling (sequential fallback)

Fragments are rare but must be handled correctly. They are processed through
the existing `PacketNormalizer` in a sequential loop:

```rust
pub fn process_fragments(
    packets: &[PcapPacket],
    fragment_indices: &[usize],
) -> Vec<OwnedNormalizedPacket> {
    if fragment_indices.is_empty() {
        return Vec::new();
    }

    let mut normalizer = PacketNormalizer::new();
    let mut result = Vec::new();

    for &idx in fragment_indices {
        let pkt = &packets[idx];
        match normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data) {
            Ok(Some(normalized)) => {
                result.push(OwnedNormalizedPacket::from_borrowed(&normalized));
            }
            Ok(None) => {} // Fragment waiting for more data
            Err(e) => {
                tracing::warn!("Fragment normalize failed: {}", e);
            }
        }
    }

    result
}
```

---

## Performance Expectations

| Scenario | Sequential | Parallel (8 cores) | Speedup |
|----------|-----------|-------------------|---------|
| 100k packets (no fragments) | ~200ms | ~30ms | ~6.7x |
| 1M packets (no fragments) | ~2s | ~300ms | ~6.7x |
| 1M packets (1% fragments) | ~2s | ~310ms | ~6.5x |

rayon overhead per task is ~50ns. With packets averaging ~500 bytes of parsing
work (~2µs), the parallelism efficiency should be >80% at 8 cores.

The fragment fallback adds negligible time since fragments are <1% of traffic.

---

## Files Changed

| File | Change |
|------|--------|
| `crates/prb-pcap/src/normalize.rs` | Add `normalize_stateless`, `NormalizeResult`, static helpers |
| `crates/prb-pcap/src/parallel/normalize.rs` | New: `NormalizeBatch`, `process_fragments` |
| `crates/prb-pcap/Cargo.toml` | Ensure `rayon = "1.11"` |

---

## Tests

- `test_normalize_stateless_tcp` — Ethernet+IPv4+TCP → correct OwnedNormalizedPacket
- `test_normalize_stateless_udp` — Ethernet+IPv4+UDP → correct transport info
- `test_normalize_stateless_fragment_detected` — Fragmented IPv4 → NormalizeResult::Fragment
- `test_normalize_stateless_all_linktypes` — Loopback, Raw IP, SLL, SLL2 each work
- `test_parallel_normalize_matches_sequential` — Same 10k packets produce identical
  results whether processed sequentially or in parallel (order-independent, sort by
  timestamp to compare)
- `test_parallel_normalize_fragment_fallback` — Mix of normal + fragmented packets;
  all fragments eventually produce normalized packets
- `test_parallel_normalize_empty_input` — Empty vec produces empty output
- `test_parallel_normalize_single_packet` — Degenerate case, still works
