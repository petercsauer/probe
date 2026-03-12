//! Criterion benchmark suite for PRB pipeline throughput.

mod fixtures;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use fixtures::SyntheticPcapBuilder;
use prb_pcap::{
    MmapPcapReader, OwnedNormalizedPacket, PacketNormalizer, PcapFileReader, TcpReassembler,
    parallel::{FlowPartitioner, NormalizeBatch, ParallelPipeline, PipelineConfig},
    tls::TlsKeyLog,
};
use rayon::prelude::*;
use std::io::Write;
use std::sync::Arc;

/// Helper: Parse a PCAP byte buffer into PcapPackets.
fn parse_fixture(pcap_data: &[u8]) -> Vec<prb_pcap::reader::PcapPacket> {
    let tmpfile = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmpfile.path(), pcap_data).unwrap();

    let mut reader = PcapFileReader::open(tmpfile.path()).unwrap();
    reader.read_all_packets().unwrap()
}

/// Helper: Normalize all packets in a fixture.
fn normalize_all(packets: &[prb_pcap::reader::PcapPacket]) -> Vec<OwnedNormalizedPacket> {
    let mut normalizer = PacketNormalizer::new();
    let mut normalized = Vec::new();

    for pkt in packets {
        if let Ok(Some(norm)) = normalizer.normalize(pkt.linktype, pkt.timestamp_us, &pkt.data) {
            normalized.push(OwnedNormalizedPacket::from_borrowed(&norm));
        }
    }

    normalized
}

/// Helper: Write PCAP data to a temp file.
fn write_tempfile(pcap_data: &[u8]) -> tempfile::NamedTempFile {
    let mut f = tempfile::NamedTempFile::new().unwrap();
    f.write_all(pcap_data).unwrap();
    f
}

// ============================================================================
// Benchmark: Packet Normalization (Sequential vs Parallel)
// ============================================================================

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
                if normalizer
                    .normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
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

    group.bench_function("parallel", |b| b.iter(|| NormalizeBatch::run(&packets)));

    group.finish();
}

// ============================================================================
// Benchmark: TCP Reassembly (Sequential vs Parallel with varying shards)
// ============================================================================

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
                let _ = reassembler.process_segment(&pkt.as_normalized());
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
                    let partitions = partitioner.partition(normalized.clone());
                    partitions
                        .into_par_iter()
                        .map(|shard| {
                            let mut r = TcpReassembler::new();
                            for pkt in &shard {
                                let borrowed = pkt.as_normalized();
                                let _ = r.process_segment(&borrowed);
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

// ============================================================================
// Benchmark: End-to-End Pipeline (Sequential vs Parallel)
// ============================================================================

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
                let mut reader = PcapFileReader::open(tmpfile.path()).unwrap();
                let packets = reader.read_all_packets().unwrap();

                // Process sequentially through normalizer
                let mut normalizer = PacketNormalizer::new();
                let mut count = 0;
                for pkt in packets {
                    if normalizer
                        .normalize(pkt.linktype, pkt.timestamp_us, &pkt.data)
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

        for jobs in [2, 4, 8] {
            group.bench_with_input(BenchmarkId::new("parallel", jobs), &jobs, |b, &jobs| {
                b.iter(|| {
                    let tmpfile = write_tempfile(&fixture);
                    let config = PipelineConfig {
                        jobs,
                        ..Default::default()
                    };
                    let pipeline = ParallelPipeline::new(
                        config,
                        tmpfile.path().to_path_buf(),
                        Arc::new(TlsKeyLog::new()),
                    );

                    // Read and normalize packets first
                    let mut reader = PcapFileReader::open(tmpfile.path()).unwrap();
                    let pcap_packets = reader.read_all_packets().unwrap();
                    let normalized = normalize_all(&pcap_packets);

                    let events = pipeline.run(normalized).unwrap();
                    events.len()
                })
            });
        }

        group.finish();
    }
}

// ============================================================================
// Benchmark: Mmap vs Streaming File Read
// ============================================================================

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

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group!(
    benches,
    bench_normalize,
    bench_tcp_reassembly,
    bench_end_to_end,
    bench_mmap_vs_streaming,
);
criterion_main!(benches);
