//! Benchmarks for MCAP storage read/write performance.

use bytes::Bytes;
use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use prb_core::{
    DebugEvent, Direction, EventId, EventSource, NetworkAddr, Payload, Timestamp, TransportKind,
};
use prb_storage::{SessionMetadata, SessionReader, SessionWriter};
use std::collections::BTreeMap;
use tempfile::NamedTempFile;

fn create_test_event(id: u64) -> DebugEvent {
    DebugEvent {
        id: EventId::from_raw(id),
        timestamp: Timestamp::from_nanos(1_000_000_000 + id * 1000),
        source: EventSource {
            adapter: "pcap".to_string(),
            origin: "benchmark.pcap".to_string(),
            network: Some(NetworkAddr {
                src: "192.168.1.1:5000".to_string(),
                dst: "192.168.1.2:6000".to_string(),
            }),
        },
        transport: TransportKind::Grpc,
        direction: if id.is_multiple_of(2) {
            Direction::Outbound
        } else {
            Direction::Inbound
        },
        payload: Payload::Raw {
            raw: Bytes::from(vec![0u8; 256]),
        },
        metadata: {
            let mut m = BTreeMap::new();
            m.insert(
                "grpc.method".to_string(),
                "/test.Service/Method".to_string(),
            );
            m
        },
        correlation_keys: vec![],
        sequence: Some(id),
        warnings: vec![],
    }
}

fn create_test_events(count: usize) -> Vec<DebugEvent> {
    (0..count).map(|i| create_test_event(i as u64)).collect()
}

fn bench_storage_write_read(c: &mut Criterion) {
    let mut group = c.benchmark_group("storage");

    let events_100 = create_test_events(100);
    let events_1000 = create_test_events(1000);

    group.throughput(Throughput::Elements(100));
    group.bench_function("write_100_events", |b| {
        b.iter(|| {
            let tmp_file = NamedTempFile::new().unwrap();
            let file = tmp_file.reopen().unwrap();
            let metadata = SessionMetadata::new()
                .with_source_file("benchmark.pcap")
                .with_capture_tool("benchmark");
            let mut writer = SessionWriter::new(file, metadata).unwrap();
            for event in black_box(&events_100) {
                writer.write_event(event).unwrap();
            }
            writer.finish().unwrap();
        });
    });

    group.throughput(Throughput::Elements(1000));
    group.bench_function("write_1000_events", |b| {
        b.iter(|| {
            let tmp_file = NamedTempFile::new().unwrap();
            let file = tmp_file.reopen().unwrap();
            let metadata = SessionMetadata::new()
                .with_source_file("benchmark.pcap")
                .with_capture_tool("benchmark");
            let mut writer = SessionWriter::new(file, metadata).unwrap();
            for event in black_box(&events_1000) {
                writer.write_event(event).unwrap();
            }
            writer.finish().unwrap();
        });
    });

    // Pre-create file for read benchmark
    let tmp_file = NamedTempFile::new().unwrap();
    let path = tmp_file.path().to_path_buf();
    {
        let file = tmp_file.reopen().unwrap();
        let metadata = SessionMetadata::new()
            .with_source_file("benchmark.pcap")
            .with_capture_tool("benchmark");
        let mut writer = SessionWriter::new(file, metadata).unwrap();
        for event in &events_1000 {
            writer.write_event(event).unwrap();
        }
        writer.finish().unwrap();
    }

    group.throughput(Throughput::Elements(1000));
    group.bench_function("read_1000_events", |b| {
        b.iter(|| {
            let reader = SessionReader::open(black_box(&path)).unwrap();
            let mut count = 0;
            for event_result in reader.events() {
                black_box(event_result).unwrap();
                count += 1;
            }
            assert_eq!(count, 1000);
        });
    });

    group.finish();
}

criterion_group!(benches, bench_storage_write_read);
criterion_main!(benches);
