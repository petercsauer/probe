//! Integration tests for EventStore and file loaders (S7.1 & S7.2).

use prb_tui::loader::load_events;
use prb_tui::EventStore;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("fixtures")
}

#[test]
fn test_load_json_fixture() {
    let fixture = fixtures_dir().join("sample.json");
    let store = load_events(&fixture).expect("Failed to load JSON fixture");

    assert!(!store.is_empty(), "Store should contain events");
    assert!(
        store.time_range().is_some(),
        "Store should have time range"
    );
}

#[test]
fn test_load_multi_transport_json() {
    let fixture = fixtures_dir().join("multi_transport.json");
    let store = load_events(&fixture).expect("Failed to load multi-transport JSON");

    assert!(!store.is_empty(), "Store should contain events");

    // Verify we have events from different transports
    let indices = store.all_indices();
    let protocol_counts = store.protocol_counts(&indices);

    assert!(
        protocol_counts.len() >= 2,
        "Should have events from multiple protocols"
    );
}

#[test]
fn test_load_empty_json() {
    let fixture = fixtures_dir().join("empty.json");
    let store = load_events(&fixture).expect("Failed to load empty JSON");

    assert_eq!(store.len(), 0, "Empty fixture should produce empty store");
    assert!(store.is_empty());
    assert!(store.time_range().is_none());
}

#[test]
fn test_event_store_timestamp_sorting() {
    use bytes::Bytes;
    use prb_core::{
        DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind,
    };
    use std::collections::BTreeMap;

    // Create events with out-of-order timestamps
    let events = vec![
        DebugEvent {
            id: EventId::from_raw(2),
            timestamp: Timestamp::from_nanos(2000),
            source: EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            },
            transport: TransportKind::Grpc,
            direction: Direction::Inbound,
            payload: Payload::Raw {
                raw: Bytes::new(),
            },
            metadata: BTreeMap::new(),
            correlation_keys: vec![],
            sequence: None,
            warnings: vec![],
        },
        DebugEvent {
            id: EventId::from_raw(1),
            timestamp: Timestamp::from_nanos(1000),
            source: EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            },
            transport: TransportKind::Grpc,
            direction: Direction::Inbound,
            payload: Payload::Raw {
                raw: Bytes::new(),
            },
            metadata: BTreeMap::new(),
            correlation_keys: vec![],
            sequence: None,
            warnings: vec![],
        },
        DebugEvent {
            id: EventId::from_raw(3),
            timestamp: Timestamp::from_nanos(3000),
            source: EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            },
            transport: TransportKind::Grpc,
            direction: Direction::Inbound,
            payload: Payload::Raw {
                raw: Bytes::new(),
            },
            metadata: BTreeMap::new(),
            correlation_keys: vec![],
            sequence: None,
            warnings: vec![],
        },
    ];

    let store = EventStore::new(events);

    // Verify events are sorted by timestamp
    assert_eq!(store.len(), 3);
    assert_eq!(store.get(0).unwrap().timestamp.as_nanos(), 1000);
    assert_eq!(store.get(1).unwrap().timestamp.as_nanos(), 2000);
    assert_eq!(store.get(2).unwrap().timestamp.as_nanos(), 3000);

    // Verify time range
    let (start, end) = store.time_range().unwrap();
    assert_eq!(start.as_nanos(), 1000);
    assert_eq!(end.as_nanos(), 3000);
}

#[test]
fn test_event_store_filter() {
    use prb_query::Filter;

    let fixture = fixtures_dir().join("multi_transport.json");
    let store = load_events(&fixture).expect("Failed to load fixture");

    // Filter for gRPC only
    let filter = Filter::parse(r#"transport == "gRPC""#).expect("Failed to parse filter");
    let grpc_indices = store.filter_indices(&filter);

    assert!(!grpc_indices.is_empty(), "Should find gRPC events");

    // Verify all returned indices are gRPC
    for &idx in &grpc_indices {
        let event = store.get(idx).unwrap();
        assert_eq!(event.transport, prb_core::TransportKind::Grpc);
    }
}

#[test]
fn test_event_store_time_buckets() {
    let fixture = fixtures_dir().join("sample.json");
    let store = load_events(&fixture).expect("Failed to load fixture");

    let indices = store.all_indices();
    let buckets = store.time_buckets(&indices, 10);

    assert_eq!(buckets.len(), 10, "Should create 10 time buckets");

    // Total events across all buckets should equal total events
    let total: u64 = buckets.iter().sum();
    assert_eq!(total, indices.len() as u64);
}

#[test]
fn test_event_store_protocol_counts() {
    let fixture = fixtures_dir().join("multi_transport.json");
    let store = load_events(&fixture).expect("Failed to load fixture");

    let indices = store.all_indices();
    let counts = store.protocol_counts(&indices);

    assert!(!counts.is_empty(), "Should have protocol counts");

    // Verify counts are sorted by count descending
    for i in 1..counts.len() {
        assert!(
            counts[i - 1].1 >= counts[i].1,
            "Protocol counts should be sorted descending"
        );
    }

    // Total should equal total events
    let total: usize = counts.iter().map(|(_, count)| count).sum();
    assert_eq!(total, indices.len());
}

#[test]
fn test_load_pcap_basic() {
    use etherparse::{Ethernet2Header, EtherType, IpNumber, Ipv4Header, TcpHeader};

    let temp_dir = tempfile::tempdir().unwrap();
    let pcap_path = temp_dir.path().join("test.pcap");

    // Create a simple PCAP file
    let mut file = fs::File::create(&pcap_path).unwrap();

    // PCAP global header
    let header = [
        0xd4, 0xc3, 0xb2, 0xa1, // Magic number (little-endian)
        0x02, 0x00, // Version major
        0x04, 0x00, // Version minor
        0x00, 0x00, 0x00, 0x00, // Timezone offset
        0x00, 0x00, 0x00, 0x00, // Timestamp accuracy
        0xff, 0xff, 0x00, 0x00, // Snaplen (65535)
        0x01, 0x00, 0x00, 0x00, // Link-layer type (Ethernet)
    ];
    file.write_all(&header).unwrap();

    // Create a simple TCP packet
    let payload = b"test payload";
    let mut packet = Vec::new();

    let eth = Ethernet2Header {
        source: [0x00, 0x11, 0x22, 0x33, 0x44, 0x55],
        destination: [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
        ether_type: EtherType(0x0800),
    };
    eth.write(&mut packet).unwrap();

    let payload_len = (20 + payload.len()) as u16;
    let ipv4 =
        Ipv4Header::new(payload_len, 64, IpNumber(6), [192, 168, 1, 1], [10, 0, 0, 1]).unwrap();
    ipv4.write(&mut packet).unwrap();

    let mut tcp = TcpHeader::new(12345, 80, 1000, 4096);
    tcp.acknowledgment_number = 0;
    tcp.syn = false;
    tcp.ack = true;
    tcp.psh = true;
    tcp.write(&mut packet).unwrap();

    packet.extend_from_slice(payload);

    // Write packet header and data
    let ts_sec = 1700000000u32;
    let ts_usec = 0u32;
    file.write_all(&ts_sec.to_le_bytes()).unwrap();
    file.write_all(&ts_usec.to_le_bytes()).unwrap();
    file.write_all(&(packet.len() as u32).to_le_bytes())
        .unwrap();
    file.write_all(&(packet.len() as u32).to_le_bytes())
        .unwrap();
    file.write_all(&packet).unwrap();
    file.flush().unwrap();
    drop(file);

    // Load the PCAP file
    let store = load_events(&pcap_path).expect("Failed to load PCAP");

    assert!(!store.is_empty(), "PCAP should produce events");

    // Verify timestamp is correct
    let event = store.get(0).unwrap();
    assert_eq!(event.timestamp.as_nanos(), 1_700_000_000_000_000_000);
}

#[test]
fn test_load_mcap_roundtrip() {
    use bytes::Bytes;
    use prb_core::{
        DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind,
    };
    use prb_storage::{SessionMetadata, SessionWriter};
    use std::collections::BTreeMap;

    let temp_dir = tempfile::tempdir().unwrap();
    let mcap_path = temp_dir.path().join("test.mcap");

    // Create some test events
    let events = vec![
        DebugEvent {
            id: EventId::from_raw(1),
            timestamp: Timestamp::from_nanos(1000),
            source: EventSource {
                adapter: "test".into(),
                origin: "test.mcap".into(),
                network: None,
            },
            transport: TransportKind::Grpc,
            direction: Direction::Inbound,
            payload: Payload::Raw {
                raw: Bytes::from_static(b"test1"),
            },
            metadata: BTreeMap::new(),
            correlation_keys: vec![],
            sequence: None,
            warnings: vec![],
        },
        DebugEvent {
            id: EventId::from_raw(2),
            timestamp: Timestamp::from_nanos(2000),
            source: EventSource {
                adapter: "test".into(),
                origin: "test.mcap".into(),
                network: None,
            },
            transport: TransportKind::Zmq,
            direction: Direction::Outbound,
            payload: Payload::Raw {
                raw: Bytes::from_static(b"test2"),
            },
            metadata: BTreeMap::new(),
            correlation_keys: vec![],
            sequence: None,
            warnings: vec![],
        },
    ];

    // Write events to MCAP
    let file = fs::File::create(&mcap_path).unwrap();
    let mut writer = SessionWriter::new(file, SessionMetadata::new()).unwrap();
    for event in &events {
        writer.write_event(event).unwrap();
    }
    writer.finish().unwrap();

    // Load events back from MCAP
    let store = load_events(&mcap_path).expect("Failed to load MCAP");

    assert_eq!(store.len(), 2, "Should load all events from MCAP");
    assert_eq!(store.get(0).unwrap().transport, TransportKind::Grpc);
    assert_eq!(store.get(1).unwrap().transport, TransportKind::Zmq);
}

#[test]
fn test_format_detection() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Test JSON detection
    let json_path = temp_dir.path().join("test.json");
    fs::write(&json_path, r#"{"version": 1, "events": []}"#).unwrap();
    let store = load_events(&json_path).expect("Failed to load JSON");
    assert_eq!(store.len(), 0);

    // Test MCAP detection
    let mcap_path = temp_dir.path().join("test.mcap");
    let file = fs::File::create(&mcap_path).unwrap();
    let writer =
        prb_storage::SessionWriter::new(file, prb_storage::SessionMetadata::new()).unwrap();
    writer.finish().unwrap();

    let store = load_events(&mcap_path).expect("Failed to load MCAP");
    assert_eq!(store.len(), 0);
}

#[test]
fn test_load_nonexistent_file() {
    let result = load_events(&PathBuf::from("/nonexistent/file.json"));
    assert!(result.is_err(), "Should fail for nonexistent file");
}

#[test]
fn test_load_invalid_format() {
    let temp_dir = tempfile::tempdir().unwrap();
    let invalid_path = temp_dir.path().join("test.xyz");
    fs::write(&invalid_path, "invalid content").unwrap();

    let result = load_events(&invalid_path);
    assert!(
        result.is_err(),
        "Should fail for unknown/unsupported format"
    );
}

#[test]
fn test_event_store_large_dataset() {
    use bytes::Bytes;
    use prb_core::{
        DebugEvent, Direction, EventId, EventSource, Payload, Timestamp, TransportKind,
    };
    use std::collections::BTreeMap;

    // Create 1000 events
    let events: Vec<DebugEvent> = (0..1000)
        .map(|i| DebugEvent {
            id: EventId::from_raw(i),
            timestamp: Timestamp::from_nanos(1000 * i),
            source: EventSource {
                adapter: "test".into(),
                origin: "test".into(),
                network: None,
            },
            transport: if i % 2 == 0 {
                TransportKind::Grpc
            } else {
                TransportKind::Zmq
            },
            direction: Direction::Inbound,
            payload: Payload::Raw {
                raw: Bytes::new(),
            },
            metadata: BTreeMap::new(),
            correlation_keys: vec![],
            sequence: None,
            warnings: vec![],
        })
        .collect();

    let store = EventStore::new(events);

    assert_eq!(store.len(), 1000);

    // Test filtering performance on large dataset
    let filter = prb_query::Filter::parse(r#"transport == "gRPC""#).unwrap();
    let indices = store.filter_indices(&filter);
    assert_eq!(indices.len(), 500, "Should find 500 gRPC events");

    // Test time bucketing
    let buckets = store.time_buckets(&store.all_indices(), 50);
    assert_eq!(buckets.len(), 50);
}
