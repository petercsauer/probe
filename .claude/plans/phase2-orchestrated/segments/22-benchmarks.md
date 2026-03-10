---
segment: 22
title: "Benchmarking Infrastructure"
depends_on: [21]
risk: 9
complexity: Low
cycle_budget: 2
status: pending
commit_message: "test: add criterion benchmark suite for pipeline throughput"
---

# Subsection 7: Benchmarking Infrastructure

## Purpose

Establish a criterion-based benchmark suite that measures throughput at each
pipeline stage and end-to-end. Benchmarks serve three purposes:
1. Validate that parallelism actually improves performance
2. Detect regressions in CI
3. Guide optimization decisions with data

---

## Segment S7.1: Benchmark Fixtures

### Synthetic PCAP generator

Create a deterministic pcap generator that produces realistic traffic patterns
at configurable scale:

```rust
// benches/fixtures/gen.rs

pub struct SyntheticPcapBuilder {
    packets: Vec<Vec<u8>>,
    linktype: u32,
}

impl SyntheticPcapBuilder {
    pub fn new() -> Self {
        Self { packets: Vec::new(), linktype: 1 }
    }

    /// Adds N TCP packets across M flows. Each flow has packets with
    /// incrementing sequence numbers and realistic payload sizes.
    pub fn tcp_flows(mut self, num_flows: usize, packets_per_flow: usize) -> Self {
        use etherparse::PacketBuilder;

        for flow_idx in 0..num_flows {
            let src_port = 10000 + (flow_idx as u16);
            let dst_port = 50051;
            let src_ip = [10, 0, (flow_idx >> 8) as u8, (flow_idx & 0xFF) as u8];
            let dst_ip = [10, 0, 0, 1];

            let mut seq = 1000u32;
            for pkt_idx in 0..packets_per_flow {
                let payload_size = 100 + (pkt_idx % 900); // 100-999 bytes
                let payload: Vec<u8> = (0..payload_size).map(|i| (i % 256) as u8).collect();

                let builder = PacketBuilder::ethernet2(
                    [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
                    [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
                )
                .ipv4(src_ip, dst_ip, 64)
                .tcp(src_port, dst_port, seq, 65535);

                let mut packet = Vec::new();
                builder.write(&mut packet, &payload).unwrap();
                self.packets.push(packet);

                seq = seq.wrapping_add(payload_size as u32);
            }
        }
        self
    }

    /// Adds N UDP packets (e.g., simulating DDS/RTPS).
    pub fn udp_packets(mut self, count: usize) -> Self {
        use etherparse::PacketBuilder;

        for i in 0..count {
            let src_port = 7400 + (i % 100) as u16;
            let payload: Vec<u8> = vec![0x52, 0x54, 0x50, 0x53]; // "RTPS" + padding
            let builder = PacketBuilder::ethernet2(
                [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
                [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
            )
            .ipv4([10, 0, 0, 1], [239, 255, 0, 1], 64)
            .udp(src_port, 7400);

            let mut packet = Vec::new();
            builder.write(&mut packet, &payload).unwrap();
            self.packets.push(packet);
        }
        self
    }

    /// Writes the packets to a legacy pcap file in memory.
    pub fn build_pcap(&self) -> Vec<u8> {
        let mut pcap = Vec::new();

        // Global header (24 bytes)
        pcap.extend_from_slice(&0xa1b2c3d4u32.to_le_bytes()); // magic
        pcap.extend_from_slice(&2u16.to_le_bytes());           // version major
        pcap.extend_from_slice(&4u16.to_le_bytes());           // version minor
        pcap.extend_from_slice(&0i32.to_le_bytes());           // thiszone
        pcap.extend_from_slice(&0u32.to_le_bytes());           // sigfigs
        pcap.extend_from_slice(&65535u32.to_le_bytes());       // snaplen
        pcap.extend_from_slice(&self.linktype.to_le_bytes());  // network

        let mut timestamp_us = 1_710_000_000_000_000u64; // ~March 2024
        for pkt in &self.packets {
            let ts_sec = (timestamp_us / 1_000_000) as u32;
            let ts_usec = (timestamp_us % 1_000_000) as u32;
            let len = pkt.len() as u32;

            // Packet record header (16 bytes)
            pcap.extend_from_slice(&ts_sec.to_le_bytes());
            pcap.extend_from_slice(&ts_usec.to_le_bytes());
            pcap.extend_from_slice(&len.to_le_bytes()); // incl_len
            pcap.extend_from_slice(&len.to_le_bytes()); // orig_len
            pcap.extend_from_slice(pkt);

            timestamp_us += 1000; // 1ms between packets
        }

        pcap
    }

    /// Writes to a temp file and returns the path.
    pub fn write_to_tempfile(&self) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(&self.build_pcap()).unwrap();
        f
    }
}
```

### Fixture sizes

| Name | Flows | Packets | Approx Size |
|------|-------|---------|-------------|
| `small` | 10 | 1,000 | ~500KB |
| `medium` | 100 | 100,000 | ~50MB |
| `large` | 1,000 | 1,000,000 | ~500MB |
| `xlarge` | 10,000 | 10,000,000 | ~5GB |

`small` and `medium` are generated in benchmark setup.
`large` and `xlarge` are generated once and cached.

---

## Segment S7.2: Criterion Benchmark Suite

```rust
// benches/pipeline_throughput.rs

use criterion::{
    criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};

fn bench_normalize(c: &mut Criterion) {
    let fixture = SyntheticPcapBuilder::new()
        .tcp_flows(100, 1000)
        .build_pcap();

    // Parse into PcapPackets first
    let packets = parse_fixture(&fixture);

    let mut group = c.benchmark_group("normalize");
    group.throughput(Throughput::Elements(packets.len() as u64));

    group.bench_function("sequential", |b| {
        b.iter(|| {
            let mut normalizer = PacketNormalizer::new();
            let mut count = 0;
            for pkt in &packets {
                if normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
                    .ok()
                    .flatten()
                    .is_some()
                {
                    count += 1;
                }
            }
            count
        })
    });

    group.bench_function("parallel", |b| {
        b.iter(|| {
            NormalizeBatch::run(&packets)
        })
    });

    group.finish();
}

fn bench_tcp_reassembly(c: &mut Criterion) {
    let fixture = SyntheticPcapBuilder::new()
        .tcp_flows(100, 1000)
        .build_pcap();
    let packets = parse_fixture(&fixture);

    // Pre-normalize
    let normalized = normalize_all(&packets);

    let mut group = c.benchmark_group("tcp_reassembly");
    group.throughput(Throughput::Elements(normalized.len() as u64));

    group.bench_function("sequential", |b| {
        b.iter(|| {
            let mut reassembler = TcpReassembler::new();
            for pkt in &normalized {
                let _ = reassembler.process_segment(pkt);
            }
            reassembler.flush_all()
        })
    });

    for shards in [2, 4, 8, 16] {
        group.bench_with_input(
            BenchmarkId::new("parallel", shards),
            &shards,
            |b, &shards| {
                b.iter(|| {
                    let partitioner = FlowPartitioner::new(shards);
                    let partitions = partitioner.partition(
                        normalized.iter().map(|p| OwnedNormalizedPacket::from_borrowed(p)).collect()
                    );
                    partitions
                        .into_par_iter()
                        .map(|shard| {
                            let mut r = TcpReassembler::new();
                            for pkt in &shard {
                                let _ = r.process_owned_segment(pkt);
                            }
                            r.flush_all()
                        })
                        .collect::<Vec<_>>()
                })
            },
        );
    }

    group.finish();
}

fn bench_end_to_end(c: &mut Criterion) {
    for (name, flows, ppf) in [("small", 10, 100), ("medium", 100, 1000)] {
        let fixture = SyntheticPcapBuilder::new()
            .tcp_flows(flows, ppf)
            .build_pcap();

        let total_packets = flows * ppf;

        let mut group = c.benchmark_group(format!("e2e_{}", name));
        group.throughput(Throughput::Elements(total_packets as u64));
        group.sample_size(10); // Larger fixtures need fewer samples

        group.bench_function("sequential", |b| {
            b.iter(|| {
                let tmpfile = write_tempfile(&fixture);
                let mut adapter = PcapCaptureAdapter::new(
                    tmpfile.path().to_path_buf(),
                    None,
                );
                let events: Vec<_> = adapter.ingest().collect();
                events.len()
            })
        });

        for jobs in [2, 4, 8] {
            group.bench_with_input(
                BenchmarkId::new("parallel", jobs),
                &jobs,
                |b, &jobs| {
                    b.iter(|| {
                        let tmpfile = write_tempfile(&fixture);
                        let config = PipelineConfig {
                            jobs,
                            ..Default::default()
                        };
                        let pipeline = ParallelPipeline::new(
                            config,
                            Arc::new(TlsKeyLog::new()),
                            tmpfile.path().to_path_buf(),
                        );
                        let packets = parse_file(tmpfile.path());
                        let events = pipeline.run(packets).unwrap();
                        events.len()
                    })
                },
            );
        }

        group.finish();
    }
}

fn bench_mmap_vs_streaming(c: &mut Criterion) {
    let fixture = SyntheticPcapBuilder::new()
        .tcp_flows(100, 1000)
        .build_pcap();

    let tmpfile = write_tempfile(&fixture);

    let mut group = c.benchmark_group("file_read");
    group.throughput(Throughput::Bytes(fixture.len() as u64));

    group.bench_function("streaming_reader", |b| {
        b.iter(|| {
            let mut reader = PcapFileReader::open(tmpfile.path()).unwrap();
            reader.read_all_packets().unwrap().len()
        })
    });

    group.bench_function("mmap_reader", |b| {
        b.iter(|| {
            let reader = MmapPcapReader::open(tmpfile.path()).unwrap();
            reader.packet_count()
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_normalize,
    bench_tcp_reassembly,
    bench_end_to_end,
    bench_mmap_vs_streaming,
);
criterion_main!(benches);
```

### Cargo.toml addition

```toml
# Workspace root Cargo.toml

[[bench]]
name = "pipeline_throughput"
harness = false
path = "benches/pipeline_throughput.rs"

[dev-dependencies]
criterion = { version = "0.8", features = ["html_reports"] }
```

---

## Benchmark Execution

```bash
# Run all benchmarks
cargo bench --bench pipeline_throughput

# Run specific group
cargo bench --bench pipeline_throughput -- normalize

# Compare against baseline (first run saves baseline)
cargo bench --bench pipeline_throughput -- --save-baseline parallel-v1

# Compare two runs
cargo bench --bench pipeline_throughput -- --baseline sequential-v1
```

### CI integration

Add to CI pipeline:
```yaml
- name: Benchmark
  run: cargo bench --bench pipeline_throughput -- --output-format bencher | tee bench-output.txt
```

Use `criterion`'s `--save-baseline` to detect regressions:
- >5% slowdown on any benchmark → CI warning
- >15% slowdown → CI failure

---

## Expected Results

| Benchmark | Sequential | 4 cores | 8 cores |
|-----------|-----------|---------|---------|
| normalize/100k | ~200ms | ~55ms | ~30ms |
| tcp_reassembly/100k/4-shards | ~300ms | ~85ms | — |
| tcp_reassembly/100k/8-shards | ~300ms | — | ~45ms |
| e2e_medium (100 flows, 100k pkts) | ~800ms | ~250ms | ~150ms |
| file_read/mmap vs stream | ~150ms | ~5ms (index only) | — |

Speedup factor of 3-7x at 8 cores, accounting for:
- rayon thread pool overhead (~2ms startup)
- Work imbalance across shards (some flows larger)
- Sequential phases (fragment defrag, final merge sort)
- Memory allocation overhead for OwnedNormalizedPacket copies

---

## Files Changed

| File | Change |
|------|--------|
| `benches/pipeline_throughput.rs` | New: full criterion benchmark suite |
| `benches/fixtures/gen.rs` | New: `SyntheticPcapBuilder` |
| `Cargo.toml` | Add `[[bench]]` section + criterion dev-dependency |

---

## Tests

Benchmarks are not tests, but the fixture generator should be tested:

- `test_synthetic_pcap_valid` — Generated pcap can be read by `PcapFileReader`
- `test_synthetic_pcap_packet_count` — N flows × M packets → N*M packets in file
- `test_synthetic_pcap_deterministic` — Same parameters → byte-identical output
