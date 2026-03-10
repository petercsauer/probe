---
segment: 17
title: "Memory-Mapped PCAP Reader"
depends_on: [16]
risk: 9
complexity: Medium
cycle_budget: 3
status: pending
commit_message: "feat(prb-pcap): add memmap2-based zero-copy PCAP/pcapng reader"
---

# Subsection 5: Memory-Mapped PCAP Reader

## Purpose

Replace the eager `read_all_packets()` approach — which loads the entire capture
file into `Vec<PcapPacket>` before processing — with a memory-mapped reader that
provides constant-memory access to arbitrarily large files. The OS manages paging,
so a 10GB pcap uses only a few MB of resident memory.

---

## Problem Statement

Current `PcapFileReader::read_all_packets()`:
1. Allocates `Vec<PcapPacket>` — each packet owns a `Vec<u8>` copy of its data
2. For a 1GB pcap with 1M packets averaging 1KB each:
   - Raw file: 1GB
   - Vec<PcapPacket>: ~1GB (data) + 24M (Vec overhead) + 16M (metadata) ≈ **~1.04GB**
   - Peak memory: **~2GB** (file buffer + parsed packets coexist during read)
3. Entire file must be read before pipeline starts — no overlap of I/O and compute

### Target

- Constant memory usage regardless of file size (~50MB working set)
- I/O and compute can overlap (mmap faults pull pages on demand)
- Zero-copy where possible (avoid copying packet data until needed)

---

## Segment S5.1: Memory-Mapped File Backend

### Design

```rust
// crates/prb-pcap/src/mmap_reader.rs

use memmap2::Mmap;
use std::fs::File;
use std::path::Path;

pub struct MmapPcapReader {
    mmap: Mmap,
    format: PcapFormat,
    packet_offsets: Vec<PacketLocation>,
}

#[derive(Debug, Clone, Copy)]
pub struct PacketLocation {
    pub offset: usize,
    pub data_offset: usize,
    pub data_len: usize,
    pub timestamp_us: u64,
    pub linktype: u32,
}

enum PcapFormat {
    Legacy { linktype: u32 },
    PcapNg { interfaces: Vec<u32> },
}
```

### Two-phase approach

**Phase 1: Index scan** — Walk the file linearly to build `Vec<PacketLocation>`.
This is fast because it only reads headers (16-28 bytes per packet), not payload
data. For a 1GB file with 1M packets, the index is ~24MB.

**Phase 2: Random access** — `PacketLocation` points into the mmap. When a
packet's data is needed, it's a zero-copy slice: `&mmap[loc.data_offset..loc.data_offset + loc.data_len]`.

```rust
impl MmapPcapReader {
    pub fn open(path: &Path) -> Result<Self, PcapError> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        let (format, packet_offsets) = Self::build_index(&mmap)?;

        Ok(Self { mmap, format, packet_offsets })
    }

    fn build_index(data: &[u8]) -> Result<(PcapFormat, Vec<PacketLocation>), PcapError> {
        if data.len() < 4 {
            return Err(PcapError::Parse("file too short".into()));
        }

        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        match magic {
            0xa1b2c3d4 | 0xd4c3b2a1 => Self::index_legacy_pcap(data, magic),
            0x0a0d0d0a => Self::index_pcapng(data),
            _ => Err(PcapError::UnsupportedFormat("unknown magic".into())),
        }
    }

    /// Returns a zero-copy slice to the packet data at the given location.
    pub fn packet_data(&self, loc: &PacketLocation) -> &[u8] {
        &self.mmap[loc.data_offset..loc.data_offset + loc.data_len]
    }

    pub fn packet_count(&self) -> usize {
        self.packet_offsets.len()
    }

    pub fn locations(&self) -> &[PacketLocation] {
        &self.packet_offsets
    }
}
```

### Legacy PCAP index scan

```rust
fn index_legacy_pcap(
    data: &[u8],
    magic: u32,
) -> Result<(PcapFormat, Vec<PacketLocation>), PcapError> {
    let swapped = magic == 0xd4c3b2a1;

    let read_u32 = |offset: usize| -> u32 {
        let bytes = [data[offset], data[offset+1], data[offset+2], data[offset+3]];
        if swapped { u32::from_be_bytes(bytes) } else { u32::from_le_bytes(bytes) }
    };

    // Global header: 24 bytes
    // magic(4) + version_major(2) + version_minor(2) + thiszone(4) +
    // sigfigs(4) + snaplen(4) + network(4)
    if data.len() < 24 {
        return Err(PcapError::Parse("pcap header truncated".into()));
    }
    let linktype = read_u32(20);
    let mut offset = 24;
    let mut locations = Vec::new();

    while offset + 16 <= data.len() {
        let ts_sec = read_u32(offset) as u64;
        let ts_usec = read_u32(offset + 4) as u64;
        let incl_len = read_u32(offset + 8) as usize;
        let _orig_len = read_u32(offset + 12) as usize;

        let data_offset = offset + 16;
        if data_offset + incl_len > data.len() {
            tracing::warn!("Truncated packet at offset {}", offset);
            break;
        }

        locations.push(PacketLocation {
            offset,
            data_offset,
            data_len: incl_len,
            timestamp_us: ts_sec * 1_000_000 + ts_usec,
            linktype,
        });

        offset = data_offset + incl_len;
    }

    Ok((PcapFormat::Legacy { linktype }, locations))
}
```

### pcapng index scan

pcapng is more complex — variable-length blocks, interface description blocks,
enhanced packet blocks, etc. The index scan walks blocks and records only
Enhanced Packet Block (EPB) and Simple Packet Block (SPB) locations:

```rust
fn index_pcapng(data: &[u8]) -> Result<(PcapFormat, Vec<PacketLocation>), PcapError> {
    let mut offset = 0;
    let mut interfaces: Vec<u32> = Vec::new();
    let mut locations = Vec::new();

    while offset + 8 <= data.len() {
        let block_type = u32::from_le_bytes(
            data[offset..offset+4].try_into().unwrap()
        );
        let block_len = u32::from_le_bytes(
            data[offset+4..offset+8].try_into().unwrap()
        ) as usize;

        if block_len < 12 || offset + block_len > data.len() {
            break;
        }

        match block_type {
            0x00000001 => {
                // Interface Description Block
                if block_len >= 20 {
                    let lt = u16::from_le_bytes(
                        data[offset+8..offset+10].try_into().unwrap()
                    ) as u32;
                    interfaces.push(lt);
                }
            }
            0x00000006 => {
                // Enhanced Packet Block
                if block_len >= 32 {
                    let if_id = u32::from_le_bytes(
                        data[offset+8..offset+12].try_into().unwrap()
                    );
                    let ts_high = u32::from_le_bytes(
                        data[offset+12..offset+16].try_into().unwrap()
                    );
                    let ts_low = u32::from_le_bytes(
                        data[offset+16..offset+20].try_into().unwrap()
                    );
                    let captured_len = u32::from_le_bytes(
                        data[offset+20..offset+24].try_into().unwrap()
                    ) as usize;

                    let linktype = interfaces.get(if_id as usize)
                        .copied().unwrap_or(1);
                    let timestamp_us = ((ts_high as u64) << 32) | (ts_low as u64);
                    let data_offset = offset + 28;

                    if data_offset + captured_len <= data.len() {
                        locations.push(PacketLocation {
                            offset,
                            data_offset,
                            data_len: captured_len,
                            timestamp_us,
                            linktype,
                        });
                    }
                }
            }
            0x0000000A => {
                // Decryption Secrets Block — record offset for TLS key extraction
                // (handled separately, not a packet)
            }
            _ => {} // Skip other block types
        }

        offset += block_len;
        // pcapng blocks are padded to 4-byte boundaries
        offset = (offset + 3) & !3;
    }

    Ok((PcapFormat::PcapNg { interfaces }, locations))
}
```

---

## Segment S5.2: Integration with Parallel Pipeline

The mmap reader produces `PacketLocation` indices instead of `PcapPacket`
objects. The parallel normalization stage accesses data directly from the mmap:

```rust
// In parallel normalize
let results: Vec<_> = reader.locations()
    .par_iter()
    .map(|loc| {
        let data = reader.packet_data(loc);
        normalize_stateless(loc.linktype, loc.timestamp_us, data)
    })
    .collect();
```

This is zero-copy until normalization extracts the transport payload (which
must be copied into `OwnedNormalizedPacket` for cross-thread transfer through
later stages).

### Fallback to streaming reader

For captures that can't be mmap'd (stdin pipe, network stream), the existing
`PcapFileReader` remains available. The pipeline auto-selects:

```rust
pub enum PacketSource {
    Mmap(MmapPcapReader),
    Stream(PcapFileReader),
}

impl PacketSource {
    pub fn open(path: &Path) -> Result<Self, PcapError> {
        // Try mmap first (faster, constant memory)
        match MmapPcapReader::open(path) {
            Ok(reader) => Ok(Self::Mmap(reader)),
            Err(_) => {
                // Fallback to streaming reader
                let reader = PcapFileReader::open(path)?;
                Ok(Self::Stream(reader))
            }
        }
    }
}
```

---

## Memory Usage Comparison

| Scenario | Current (eager) | mmap reader |
|----------|----------------|-------------|
| 100MB pcap | ~200MB peak | ~25MB (index + working set) |
| 1GB pcap | ~2GB peak | ~50MB (index + working set) |
| 10GB pcap | OOM likely | ~200MB (index + working set) |

The mmap approach has near-constant memory: the index grows linearly with
packet count (~24 bytes per packet), but the actual packet data is paged in
and out by the OS. Typical working set is 10-50MB regardless of file size.

---

## Safety

`memmap2::Mmap::map()` is `unsafe` because:
1. Another process could modify the file while mapped (UB)
2. File could be truncated, causing SIGBUS

Mitigations:
- Open file read-only + advisory file lock (`flock`)
- Wrap mmap access in bounds checks (PacketLocation validated during index scan)
- Document that concurrent file modification is unsupported

This is the same safety model used by every mmap-based database (SQLite WAL,
LMDB, RocksDB).

---

## Files Changed

| File | Change |
|------|--------|
| `crates/prb-pcap/src/mmap_reader.rs` | New: `MmapPcapReader`, `PacketLocation`, index scanners |
| `crates/prb-pcap/src/lib.rs` | Add `pub mod mmap_reader;` |
| `crates/prb-pcap/Cargo.toml` | Add `memmap2 = "0.9"` |
| `crates/prb-pcap/src/parallel/orchestrator.rs` | Use `PacketSource` abstraction |

---

## Tests

- `test_mmap_legacy_pcap_index` — Index a legacy pcap, verify packet count
  and locations match streaming reader
- `test_mmap_pcapng_index` — Index a pcapng, verify IDB linktype tracking
  and EPB locations
- `test_mmap_zero_copy_data` — `packet_data()` returns correct bytes for
  each location
- `test_mmap_truncated_file` — Truncated file stops indexing at truncation
  point without panic
- `test_mmap_empty_file` — 24-byte header, zero packets → empty index
- `test_mmap_matches_streaming` — Same file read via mmap and streaming reader
  produces identical packet data (byte-for-byte comparison)
- `test_mmap_large_file_constant_memory` — Process 100MB fixture, assert RSS
  stays under 50MB (platform-dependent, may need `#[ignore]`)
